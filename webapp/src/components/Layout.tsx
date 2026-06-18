import { Link, useNavigate } from 'react-router-dom'
import { clearSession } from '../api'

export default function Layout({ children }: { children: React.ReactNode }) {
  const nav = useNavigate()
  function logout() {
    clearSession()
    nav('/login')
  }
  return (
    <div className="min-h-screen flex">
      <aside className="w-56 bg-gray-900 border-r border-gray-800 flex flex-col px-4 py-6 gap-2">
        <Link to="/" className="text-white font-bold text-lg mb-6 flex items-center gap-2">
          <span className="text-indigo-400">&#9670;</span> SDMemory
        </Link>
        <Link to="/" className="text-gray-400 hover:text-white text-sm px-2 py-1.5 rounded hover:bg-gray-800 transition">
          Dashboard
        </Link>
        <div className="flex-1" />
        <button onClick={logout} className="text-gray-500 hover:text-white text-sm px-2 py-1.5 rounded hover:bg-gray-800 transition text-left">
          Sign out
        </button>
      </aside>
      <main className="flex-1 p-8 overflow-auto">{children}</main>
    </div>
  )
}
