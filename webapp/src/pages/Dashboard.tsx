import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { listApps, createApp, orgStats, getOrgId, App } from '../api'
import Layout from '../components/Layout'

export default function Dashboard() {
  const orgId = getOrgId()
  const nav = useNavigate()
  const [apps, setApps] = useState<App[]>([])
  const [stats, setStats] = useState({ total_apps: 0, total_users: 0, total_memories: 0 })
  const [newAppName, setNewAppName] = useState('')
  const [newApiKey, setNewApiKey] = useState('')
  const [creating, setCreating] = useState(false)
  const [err, setErr] = useState('')

  async function load() {
    try {
      const [a, s] = await Promise.all([listApps(orgId), orgStats(orgId)])
      setApps(a.apps)
      setStats(s.stats)
    } catch (e: unknown) {
      setErr(e instanceof Error ? e.message : String(e))
    }
  }

  useEffect(() => { load() }, [])

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault()
    if (!newAppName.trim()) return
    setCreating(true)
    setErr('')
    try {
      const { app, api_key } = await createApp(orgId, newAppName.trim())
      setNewApiKey(api_key)
      setNewAppName('')
      setApps(prev => [...prev, app])
      setStats(prev => ({ ...prev, total_apps: prev.total_apps + 1 }))
    } catch (e: unknown) {
      setErr(e instanceof Error ? e.message : String(e))
    } finally {
      setCreating(false)
    }
  }

  return (
    <Layout>
      <h1 className="text-xl font-semibold text-white mb-6">Dashboard</h1>

      {/* Stats */}
      <div className="grid grid-cols-3 gap-4 mb-8">
        {[
          { label: 'Apps', value: stats.total_apps },
          { label: 'Users', value: stats.total_users },
          { label: 'Memories', value: stats.total_memories },
        ].map(s => (
          <div key={s.label} className="bg-gray-900 border border-gray-800 rounded-lg p-4">
            <p className="text-gray-400 text-xs mb-1">{s.label}</p>
            <p className="text-2xl font-bold text-white">{s.value}</p>
          </div>
        ))}
      </div>

      {/* New API key reveal */}
      {newApiKey && (
        <div className="mb-6 bg-indigo-950 border border-indigo-700 rounded-lg p-4">
          <p className="text-indigo-300 text-sm font-medium mb-1">App created — save your API key now</p>
          <p className="text-xs text-gray-400 mb-2">It won't be shown again.</p>
          <div className="flex items-center gap-2">
            <code className="flex-1 bg-gray-900 rounded px-3 py-2 text-xs text-green-400 font-mono break-all">
              {newApiKey}
            </code>
            <button
              onClick={() => { navigator.clipboard.writeText(newApiKey); }}
              className="text-xs px-3 py-2 bg-indigo-600 hover:bg-indigo-700 rounded text-white transition"
            >
              Copy
            </button>
            <button
              onClick={() => setNewApiKey('')}
              className="text-xs px-3 py-2 bg-gray-700 hover:bg-gray-600 rounded text-white transition"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* Apps list */}
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-medium text-gray-300">Apps</h2>
      </div>

      {err && <p className="text-red-400 text-xs mb-4">{err}</p>}

      <div className="space-y-2 mb-6">
        {apps.length === 0 && (
          <p className="text-gray-500 text-sm">No apps yet. Create one below.</p>
        )}
        {apps.map(app => (
          <button
            key={app.id}
            onClick={() => nav(`/apps/${app.id}`)}
            className="w-full text-left bg-gray-900 border border-gray-800 hover:border-gray-700 rounded-lg px-4 py-3 flex items-center justify-between transition"
          >
            <div>
              <p className="text-white text-sm font-medium">{app.name}</p>
              <p className="text-gray-500 text-xs mt-0.5">{app.id}</p>
            </div>
            <span className="text-gray-600 text-sm">&#8250;</span>
          </button>
        ))}
      </div>

      {/* Create app */}
      <form onSubmit={handleCreate} className="flex gap-2">
        <input
          className="flex-1 bg-gray-900 border border-gray-800 rounded px-3 py-2 text-sm text-white placeholder-gray-600 focus:outline-none focus:border-indigo-500"
          value={newAppName}
          onChange={e => setNewAppName(e.target.value)}
          placeholder="New app name..."
        />
        <button
          type="submit"
          disabled={creating}
          className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 rounded text-sm font-medium text-white transition"
        >
          {creating ? '...' : 'Create app'}
        </button>
      </form>
    </Layout>
  )
}
