import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { getAdminToken, getOrgId } from './api'
import Login from './pages/Login'
import Dashboard from './pages/Dashboard'
import AppDetail from './pages/AppDetail'

function RequireAuth({ children }: { children: JSX.Element }) {
  if (!getAdminToken() || !getOrgId()) {
    return <Navigate to="/login" replace />
  }
  return children
}

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route
          path="/"
          element={
            <RequireAuth>
              <Dashboard />
            </RequireAuth>
          }
        />
        <Route
          path="/apps/:appId"
          element={
            <RequireAuth>
              <AppDetail />
            </RequireAuth>
          }
        />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
