import type { ReactElement } from "react"
import { BrowserRouter as Router, Routes, Route, Link, Navigate, useLocation } from "react-router-dom"
import SkillList from "./pages/SkillList"
import SkillDetail from "./pages/SkillDetail"
import Login from "./pages/Login"
import Register from "./pages/Register"
import AuthCallback from "./pages/AuthCallback"
import DiscoveryRegistries from "./pages/DiscoveryRegistries"
import { AuthProvider, useAuth } from "./contexts/AuthContext"

function RequireAdmin({ children }: { children: ReactElement }) {
  const { role, loading } = useAuth()
  const location = useLocation()

  if (loading) {
    return (
      <div className="container mx-auto py-8 text-sm text-muted-foreground">
        Checking admin permission...
      </div>
    )
  }

  if (role !== "admin") {
    const next = `${location.pathname}${location.search}`
    const redirect = encodeURIComponent(next)
    return <Navigate to={`/login?reason=admin_required&redirect=${redirect}`} replace />
  }

  return children
}

function AppContent() {
  const { role } = useAuth()

  return (
    <div className="min-h-screen bg-background font-sans antialiased text-foreground">
      {/* Navigation */}
      <header className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="container flex h-14 items-center">
          <div className="mr-4 hidden md:flex">
            <Link to="/" className="mr-6 flex items-center space-x-2">
              <span className="hidden font-bold sm:inline-block">
                Agent Skills
              </span>
            </Link>
            <nav className="flex items-center space-x-6 text-sm font-medium">
              <Link to="/" className="transition-colors hover:text-foreground/80 text-foreground">
                Skills
              </Link>
              <a href="https://agentskills.io" className="transition-colors hover:text-foreground/80 text-foreground/60">
                Documentation
              </a>
              {role ? (
                <>
                  <span className="text-muted-foreground">Logged in as {role}</span>
                  <Link to="/logout" className="transition-colors hover:text-foreground/80 text-foreground/60">
                    Logout
                  </Link>
                </>
              ) : (
                <Link to="/login" className="transition-colors hover:text-foreground/80 text-foreground/60">
                  Login
                </Link>
              )}
              {role === "admin" && (
                <Link to="/admin/discovery-registries" className="transition-colors hover:text-foreground/80 text-foreground/60">
                  Discovery Registries
                </Link>
              )}
            </nav>
          </div>
          <div className="flex flex-1 items-center justify-between space-x-2 md:justify-end">
             <div className="w-full flex-1 md:w-auto md:flex-none">
                {/* Global search could go here */}
             </div>
          </div>
        </div>
      </header>

      <main>
        <Routes>
          <Route path="/" element={<SkillList />} />
          <Route path="/login" element={<Login />} />
          <Route path="/register" element={<Register />} />
          <Route path="/auth/callback" element={<AuthCallback />} />
          <Route
            path="/admin/discovery-registries"
            element={(
              <RequireAdmin>
                <DiscoveryRegistries />
              </RequireAdmin>
            )}
          />
          <Route path="/:owner" element={<SkillList />} />
          <Route path="/:owner/:repo" element={<SkillList />} />
          <Route path="/:owner/:repo/:name" element={<SkillDetail />} />
        </Routes>
      </main>
    </div>
  )
}

function App() {
  return (
    <Router>
      <AuthProvider>
        <AppContent />
      </AuthProvider>
    </Router>
  )
}

export default App
