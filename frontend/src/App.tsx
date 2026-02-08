import { useEffect, useState, type ReactElement } from "react"
import { BrowserRouter as Router, Routes, Route, Link, Navigate, useLocation, useNavigate } from "react-router-dom"
import { BookOpen, Menu, X } from "lucide-react"
import SkillList from "./pages/SkillList"
import SkillDetail from "./pages/SkillDetail"
import Login from "./pages/Login"
import Register from "./pages/Register"
import AuthCallback from "./pages/AuthCallback"
import DiscoveryRegistries from "./pages/DiscoveryRegistries"
import { AuthProvider, useAuth } from "./contexts/AuthContext"
import { Button } from "./components/ui/button"

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
  const { role, user, logout } = useAuth()
  const location = useLocation()
  const navigate = useNavigate()
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)

  useEffect(() => {
    setMobileMenuOpen(false)
  }, [location.pathname, location.search])

  const onLogout = async () => {
    await logout()
    navigate("/")
  }

  const userLabel = user?.display_name || user?.username || role

  return (
    <div className="min-h-screen bg-background font-sans antialiased text-foreground">
      <header className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="container flex h-14 items-center justify-between gap-4">
          <div className="flex items-center gap-6">
            <Link to="/" className="font-bold">
              Agent Skills
            </Link>
            <nav className="hidden items-center gap-5 text-sm font-medium md:flex">
              <Link to="/" className="transition-colors hover:text-foreground/80 text-foreground">
                Skills
              </Link>
              {role === "admin" && (
                <Link to="/admin/discovery-registries" className="transition-colors hover:text-foreground/80 text-foreground/60">
                  Discovery Registries
                </Link>
              )}
            </nav>
          </div>

          <div className="hidden items-center gap-3 md:flex">
            <Button asChild variant="ghost" size="icon">
              <a
                href="https://agentskills.io"
                aria-label="Documentation"
                title="Documentation"
              >
                <BookOpen className="h-4 w-4 text-muted-foreground" />
              </a>
            </Button>
            {role ? (
              <>
                <span className="rounded-full border bg-muted px-2.5 py-1 text-xs text-muted-foreground">
                  {userLabel}
                </span>
                <Button variant="ghost" size="sm" onClick={onLogout}>
                  Logout
                </Button>
              </>
            ) : (
              <Button asChild variant="ghost" size="sm">
                <Link to="/login">Login</Link>
              </Button>
            )}
          </div>

          <Button
            variant="ghost"
            size="icon"
            className="md:hidden"
            aria-label={mobileMenuOpen ? "Close menu" : "Open menu"}
            onClick={() => setMobileMenuOpen((prev) => !prev)}
          >
            {mobileMenuOpen ? <X className="h-5 w-5" /> : <Menu className="h-5 w-5" />}
          </Button>
        </div>

        {mobileMenuOpen && (
          <div className="border-t bg-background md:hidden">
            <div className="container space-y-2 py-3">
              <Link to="/" className="block rounded-md px-2 py-2 text-sm font-medium hover:bg-accent">
                Skills
              </Link>
              <a href="https://agentskills.io" className="block rounded-md px-2 py-2 text-sm font-medium hover:bg-accent">
                Documentation
              </a>
              {role === "admin" && (
                <Link to="/admin/discovery-registries" className="block rounded-md px-2 py-2 text-sm font-medium hover:bg-accent">
                  Discovery Registries
                </Link>
              )}
              <div className="pt-2">
                {role ? (
                  <div className="flex items-center justify-between gap-3 rounded-md border bg-muted/40 px-3 py-2">
                    <span className="text-sm text-muted-foreground">Signed in as {userLabel}</span>
                    <Button variant="outline" size="sm" onClick={onLogout}>
                      Logout
                    </Button>
                  </div>
                ) : (
                  <Button asChild className="w-full" variant="outline">
                    <Link to="/login">Login</Link>
                  </Button>
                )}
              </div>
            </div>
          </div>
        )}
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
          <Route path="/:host" element={<SkillList />} />
          <Route path="/:host/:org" element={<SkillList />} />
          <Route path="/:host/:org/:repo" element={<SkillList />} />
          <Route path="/:host/:org/:repo/skill/:name" element={<SkillDetail />} />
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
