import { useEffect, useState } from 'react'
import { useParams } from 'react-router-dom'
import { listAppUsers, UserWithCount } from '../api'
import Layout from '../components/Layout'
import CopyButton from '../components/CopyButton'

export default function AppDetail() {
  const { appId } = useParams<{ appId: string }>()
  const [apiKey, setApiKey] = useState('')
  const [users, setUsers] = useState<UserWithCount[]>([])
  const [err, setErr] = useState('')

  async function loadUsers(key: string) {
    if (!key || !appId) return
    try {
      const { users } = await listAppUsers(appId, key)
      setUsers(users)
    } catch (e: unknown) {
      setErr(e instanceof Error ? e.message : String(e))
    }
  }

  useEffect(() => {
    const stored = localStorage.getItem(`sdm_apikey_${appId}`)
    if (stored) { setApiKey(stored); loadUsers(stored) }
  }, [appId])

  function handleKeySubmit(e: React.FormEvent) {
    e.preventDefault()
    localStorage.setItem(`sdm_apikey_${appId}`, apiKey)
    loadUsers(apiKey)
  }

  const curlRemember = `curl -X POST http://localhost:3000/api/v1/memories \\
  -H "Authorization: Bearer YOUR_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"user_id": "alice@example.com", "text": "Alice loves coffee"}'`

  const curlRecall = `curl "http://localhost:3000/api/v1/memories?user_id=alice@example.com&q=coffee" \\
  -H "Authorization: Bearer YOUR_API_KEY"`

  return (
    <Layout>
      <div className="max-w-3xl">
        <h1 className="text-xl font-semibold text-white mb-1">App</h1>
        <p className="text-gray-500 text-xs mb-6">{appId}</p>

        {/* API key entry */}
        <div className="bg-gray-900 border border-gray-800 rounded-lg p-4 mb-6">
          <p className="text-sm font-medium text-gray-300 mb-3">API Key</p>
          <form onSubmit={handleKeySubmit} className="flex gap-2">
            <input
              type="password"
              className="flex-1 bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white focus:outline-none focus:border-indigo-500"
              value={apiKey}
              onChange={e => setApiKey(e.target.value)}
              placeholder="Paste your API key to view users"
            />
            <button
              type="submit"
              className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 rounded text-sm font-medium text-white transition"
            >
              Load
            </button>
          </form>
        </div>

        {/* Users */}
        <div className="mb-8">
          <h2 className="text-sm font-medium text-gray-300 mb-3">Users</h2>
          {err && <p className="text-red-400 text-xs mb-3">{err}</p>}
          {users.length === 0 ? (
            <p className="text-gray-500 text-sm">No users yet. Start storing memories via the API.</p>
          ) : (
            <div className="space-y-2">
              {users.map(u => (
                <div
                  key={u.user.id}
                  className="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 flex items-center justify-between"
                >
                  <div>
                    <p className="text-white text-sm">{u.user.external_user_id}</p>
                    <p className="text-gray-500 text-xs mt-0.5">since {new Date(u.user.created_at).toLocaleDateString()}</p>
                  </div>
                  <span className="text-indigo-400 text-sm font-medium">{u.memory_count} memories</span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Integration docs */}
        <div>
          <h2 className="text-sm font-medium text-gray-300 mb-3">Integration</h2>
          <div className="space-y-4">
            <div className="bg-gray-900 border border-gray-800 rounded-lg p-4">
              <div className="flex items-center justify-between mb-2">
                <p className="text-xs text-gray-400">Store a memory</p>
                <CopyButton text={curlRemember} />
              </div>
              <pre className="text-xs text-green-400 font-mono whitespace-pre-wrap break-all">{curlRemember}</pre>
            </div>
            <div className="bg-gray-900 border border-gray-800 rounded-lg p-4">
              <div className="flex items-center justify-between mb-2">
                <p className="text-xs text-gray-400">Recall memories</p>
                <CopyButton text={curlRecall} />
              </div>
              <pre className="text-xs text-green-400 font-mono whitespace-pre-wrap break-all">{curlRecall}</pre>
            </div>
          </div>
        </div>
      </div>
    </Layout>
  )
}
