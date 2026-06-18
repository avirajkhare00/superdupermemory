import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  createOrg,
  setAdminToken,
  setOrgId,
  getAdminToken,
  getOrgId,
} from '../api'

export default function Login() {
  const nav = useNavigate()
  const [mode, setMode] = useState<'login' | 'setup'>('login')
  const [token, setToken] = useState('')
  const [orgId, setOrgIdLocal] = useState('')
  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')
  const [err, setErr] = useState('')
  const [loading, setLoading] = useState(false)

  // Already logged in
  if (getAdminToken() && getOrgId()) {
    nav('/')
    return null
  }

  async function login(e: React.FormEvent) {
    e.preventDefault()
    setErr('')
    setAdminToken(token.trim())
    setOrgId(orgId.trim())
    nav('/')
  }

  async function setup(e: React.FormEvent) {
    e.preventDefault()
    setErr('')
    setLoading(true)
    try {
      const { org, admin_token } = await createOrg(name.trim(), slug.trim())
      setAdminToken(admin_token)
      setOrgId(org.id)
      nav('/')
    } catch (e: unknown) {
      setErr(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <span className="text-indigo-400 text-4xl">&#9670;</span>
          <h1 className="text-2xl font-bold text-white mt-2">Superdupermemory</h1>
          <p className="text-gray-400 text-sm mt-1">Local-first memory layer for AI agents</p>
        </div>

        <div className="bg-gray-900 rounded-xl border border-gray-800 p-6">
          <div className="flex gap-2 mb-6">
            <button
              onClick={() => setMode('login')}
              className={`flex-1 py-1.5 rounded text-sm font-medium transition ${
                mode === 'login' ? 'bg-indigo-600 text-white' : 'text-gray-400 hover:text-white'
              }`}
            >
              Sign in
            </button>
            <button
              onClick={() => setMode('setup')}
              className={`flex-1 py-1.5 rounded text-sm font-medium transition ${
                mode === 'setup' ? 'bg-indigo-600 text-white' : 'text-gray-400 hover:text-white'
              }`}
            >
              First-time setup
            </button>
          </div>

          {mode === 'login' ? (
            <form onSubmit={login} className="space-y-4">
              <div>
                <label className="text-xs text-gray-400 mb-1 block">Org ID</label>
                <input
                  className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white focus:outline-none focus:border-indigo-500"
                  value={orgId}
                  onChange={e => setOrgIdLocal(e.target.value)}
                  placeholder="org id from setup"
                  required
                />
              </div>
              <div>
                <label className="text-xs text-gray-400 mb-1 block">Admin Token</label>
                <input
                  type="password"
                  className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white focus:outline-none focus:border-indigo-500"
                  value={token}
                  onChange={e => setToken(e.target.value)}
                  placeholder="your admin token"
                  required
                />
              </div>
              <button
                type="submit"
                className="w-full py-2 bg-indigo-600 hover:bg-indigo-700 rounded text-sm font-medium text-white transition"
              >
                Sign in
              </button>
            </form>
          ) : (
            <form onSubmit={setup} className="space-y-4">
              <p className="text-xs text-gray-400">
                Create a new organization. Save your admin token — it won't be shown again.
              </p>
              <div>
                <label className="text-xs text-gray-400 mb-1 block">Organization name</label>
                <input
                  className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white focus:outline-none focus:border-indigo-500"
                  value={name}
                  onChange={e => setName(e.target.value)}
                  placeholder="Acme Corp"
                  required
                />
              </div>
              <div>
                <label className="text-xs text-gray-400 mb-1 block">Slug</label>
                <input
                  className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white focus:outline-none focus:border-indigo-500"
                  value={slug}
                  onChange={e => setSlug(e.target.value)}
                  placeholder="acme"
                  pattern="[a-z0-9\-]+"
                  required
                />
              </div>
              {err && <p className="text-red-400 text-xs">{err}</p>}
              <button
                type="submit"
                disabled={loading}
                className="w-full py-2 bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 rounded text-sm font-medium text-white transition"
              >
                {loading ? 'Creating...' : 'Create organization'}
              </button>
            </form>
          )}
        </div>
      </div>
    </div>
  )
}
