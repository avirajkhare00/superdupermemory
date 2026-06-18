const BASE = '/api/v1'

export function getAdminToken() {
  return localStorage.getItem('sdm_admin_token') ?? ''
}
export function setAdminToken(t: string) {
  localStorage.setItem('sdm_admin_token', t)
}
export function getOrgId() {
  return localStorage.getItem('sdm_org_id') ?? ''
}
export function setOrgId(id: string) {
  localStorage.setItem('sdm_org_id', id)
}
export function clearSession() {
  localStorage.removeItem('sdm_admin_token')
  localStorage.removeItem('sdm_org_id')
}

async function req<T>(
  path: string,
  opts: RequestInit = {},
  extraHeaders: Record<string, string> = {}
): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...opts,
    headers: {
      'Content-Type': 'application/json',
      ...extraHeaders,
      ...((opts.headers as Record<string, string>) ?? {}),
    },
  })
  const data = await res.json()
  if (!res.ok) throw new Error(data.error ?? res.statusText)
  return data as T
}

function adminHeaders() {
  return { 'X-Admin-Token': getAdminToken() }
}

// ── orgs ───────────────────────────────────────────────────────────────────

export interface Org {
  id: string
  name: string
  slug: string
  created_at: string
}

export async function createOrg(name: string, slug: string) {
  return req<{ org: Org; admin_token: string }>('/orgs', {
    method: 'POST',
    body: JSON.stringify({ name, slug }),
  })
}

// ── apps ───────────────────────────────────────────────────────────────────

export interface App {
  id: string
  org_id: string
  name: string
  created_at: string
}

export async function listApps(orgId: string) {
  return req<{ apps: App[] }>(`/orgs/${orgId}/apps`, {}, adminHeaders())
}

export async function createApp(orgId: string, name: string) {
  return req<{ app: App; api_key: string }>(
    `/orgs/${orgId}/apps`,
    { method: 'POST', body: JSON.stringify({ name }) },
    adminHeaders()
  )
}

export async function orgStats(orgId: string) {
  return req<{ stats: { total_apps: number; total_users: number; total_memories: number } }>(
    `/orgs/${orgId}/stats`,
    {},
    adminHeaders()
  )
}

// ── app users ──────────────────────────────────────────────────────────────

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

export async function listAppUsers(appId: string, apiKey: string) {
  return req<{ users: UserWithCount[] }>(
    `/apps/${appId}/users`,
    {},
    { Authorization: `Bearer ${apiKey}` }
  )
}

// ── memories ───────────────────────────────────────────────────────────────

export interface Fact {
  id: string
  subject: string
  body: string
  source: string
  created_at: string
  updated_at: string
}

export async function remember(apiKey: string, userId: string, text: string) {
  return req<{ facts: Fact[] }>(
    '/memories',
    { method: 'POST', body: JSON.stringify({ user_id: userId, text }) },
    { Authorization: `Bearer ${apiKey}` }
  )
}

export async function recall(apiKey: string, userId: string, q?: string, limit = 10) {
  const params = new URLSearchParams({ user_id: userId, limit: String(limit) })
  if (q) params.set('q', q)
  return req<{ facts: Fact[] }>(`/memories?${params}`, {}, { Authorization: `Bearer ${apiKey}` })
}
