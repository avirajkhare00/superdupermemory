// ── Types ──────────────────────────────────────────────────────────────────

export interface Fact {
  id: string
  subject: string
  body: string
  source: string
  created_at: string
  updated_at: string
  previous_body: string | null
}

export interface Org {
  id: string
  name: string
  slug: string
  created_at: string
}

export interface App {
  id: string
  org_id: string
  name: string
  created_at: string
}

export interface AppUser {
  id: string
  app_id: string
  external_user_id: string
  created_at: string
}

export interface UserWithCount {
  user: AppUser
  memory_count: number
}

export interface OrgStats {
  total_apps: number
  total_users: number
  total_memories: number
}

// ── Internal fetch helper ──────────────────────────────────────────────────

async function apiFetch<T>(
  baseUrl: string,
  path: string,
  headers: Record<string, string>,
  init: RequestInit = {}
): Promise<T> {
  const res = await fetch(`${baseUrl}/api/v1${path}`, {
    ...init,
    headers: { 'Content-Type': 'application/json', ...headers, ...(init.headers as Record<string, string> ?? {}) },
  })
  const data = await res.json()
  if (!res.ok) throw new Error((data as { error?: string }).error ?? res.statusText)
  return data as T
}

// ── SupduperMemory — memory operations for a single app ───────────────────

export interface SupduperMemoryOptions {
  /** Base URL of your superdupermemory server, e.g. "http://localhost:3000" */
  baseUrl: string
  /** API key for your app (from the dashboard or createApp) */
  apiKey: string
}

export interface RememberOptions {
  /** Your user's identifier — any string (email, UUID, username, etc.) */
  userId: string
  /** Free-form text to extract facts from */
  text: string
  /** Optional label for where this text came from */
  source?: string
}

export interface RecallOptions {
  /** Your user's identifier */
  userId: string
  /** Natural-language query — leave empty to list most recent memories */
  query?: string
  /** Max results (default 10, max 100) */
  limit?: number
}

export interface ForgetOptions {
  /** Your user's identifier */
  userId: string
  /** Fact ID returned by remember or recall */
  factId: string
}

export class SupduperMemory {
  private base: string
  private headers: Record<string, string>

  constructor(opts: SupduperMemoryOptions) {
    this.base = opts.baseUrl.replace(/\/$/, '')
    this.headers = { Authorization: `Bearer ${opts.apiKey}` }
  }

  /**
   * Extract facts from text and store them for a user.
   * Returns the list of facts that were saved.
   */
  async remember(opts: RememberOptions): Promise<Fact[]> {
    const { facts } = await apiFetch<{ facts: Fact[] }>(
      this.base, '/memories', this.headers,
      { method: 'POST', body: JSON.stringify({ user_id: opts.userId, text: opts.text, source: opts.source }) }
    )
    return facts
  }

  /**
   * Recall memories for a user using semantic search.
   * Omit `query` to list the most recent memories.
   */
  async recall(opts: RecallOptions): Promise<Fact[]> {
    const p = new URLSearchParams({ user_id: opts.userId })
    if (opts.query) p.set('q', opts.query)
    if (opts.limit) p.set('limit', String(opts.limit))
    const { facts } = await apiFetch<{ facts: Fact[] }>(this.base, `/memories?${p}`, this.headers)
    return facts
  }

  /**
   * Delete a specific memory for a user.
   * Returns true if the memory was found and deleted.
   */
  async forget(opts: ForgetOptions): Promise<boolean> {
    const p = new URLSearchParams({ user_id: opts.userId })
    const { deleted } = await apiFetch<{ deleted: boolean }>(
      this.base, `/memories/${opts.factId}?${p}`, this.headers,
      { method: 'DELETE' }
    )
    return deleted
  }

  /**
   * List all users for this app along with their memory counts.
   * Requires the app ID (available from the dashboard).
   */
  async users(appId: string): Promise<UserWithCount[]> {
    const { users } = await apiFetch<{ users: UserWithCount[] }>(
      this.base, `/apps/${appId}/users`, this.headers
    )
    return users
  }
}

// ── SupduperMemoryAdmin — org and app management ───────────────────────────

export interface SupduperMemoryAdminOptions {
  /** Base URL of your superdupermemory server */
  baseUrl: string
  /** Admin token returned when you created your org */
  adminToken: string
  /** Your org ID */
  orgId: string
}

export class SupduperMemoryAdmin {
  private base: string
  private headers: Record<string, string>
  private orgId: string

  constructor(opts: SupduperMemoryAdminOptions) {
    this.base = opts.baseUrl.replace(/\/$/, '')
    this.headers = { 'X-Admin-Token': opts.adminToken }
    this.orgId = opts.orgId
  }

  private fetch<T>(path: string, init?: RequestInit) {
    return apiFetch<T>(this.base, path, this.headers, init)
  }

  /** List all apps in your org */
  async listApps(): Promise<App[]> {
    const { apps } = await this.fetch<{ apps: App[] }>(`/orgs/${this.orgId}/apps`)
    return apps
  }

  /**
   * Create a new app.
   * The returned `apiKey` is shown only once — store it securely.
   */
  async createApp(name: string): Promise<{ app: App; apiKey: string }> {
    const { app, api_key } = await this.fetch<{ app: App; api_key: string }>(
      `/orgs/${this.orgId}/apps`,
      { method: 'POST', body: JSON.stringify({ name }) }
    )
    return { app, apiKey: api_key }
  }

  /** Get memory counts across your org */
  async stats(): Promise<OrgStats> {
    const { stats } = await this.fetch<{ stats: OrgStats }>(`/orgs/${this.orgId}/stats`)
    return stats
  }
}

// ── createOrg — one-time setup helper ─────────────────────────────────────

export interface CreateOrgOptions {
  /** Base URL of your superdupermemory server */
  baseUrl: string
  /** Display name for your organization */
  name: string
  /** URL-safe slug (lowercase letters, numbers, hyphens) */
  slug: string
}

/**
 * Create a new organization.
 * The returned `adminToken` is shown only once — store it securely.
 * Use this once during setup; after that use SupduperMemoryAdmin.
 */
export async function createOrg(opts: CreateOrgOptions): Promise<{ org: Org; adminToken: string }> {
  const base = opts.baseUrl.replace(/\/$/, '')
  const res = await fetch(`${base}/api/v1/orgs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name: opts.name, slug: opts.slug }),
  })
  const data = await res.json()
  if (!res.ok) throw new Error((data as { error?: string }).error ?? res.statusText)
  return { org: (data as { org: Org; admin_token: string }).org, adminToken: (data as { org: Org; admin_token: string }).admin_token }
}
