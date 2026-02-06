import { useEffect, useState } from "react"
import { BrowserRouter as Router, Routes, Route, Link } from "react-router-dom"
import SkillList from "./pages/SkillList"
import SkillDetail from "./pages/SkillDetail"
import Login from "./pages/Login"
import Register from "./pages/Register"
import AuthCallback from "./pages/AuthCallback"
import DiscoveryRegistries from "./pages/DiscoveryRegistries"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"

type MeResponse = {
  user_id: string
  username: string | null
  role: string
  display_name: string | null
  primary_email: string | null
}

function App() {
  const [role, setRole] = useState<string | null>(null)

  useEffect(() => {
    const fetchMe = async () => {
      try {
        const res = await api.get<ApiResponse<MeResponse>>("/api/me")
        if (res.data.code === 200) {
          setRole(res.data.data?.role ?? null)
        }
      } catch {
        setRole(null)
      }
    }

    fetchMe()
  }, [])

  return (
    <Router>
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
                <Link to="/login" className="transition-colors hover:text-foreground/80 text-foreground/60">
                  Login
                </Link>
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
            <Route path="/admin/discovery-registries" element={<DiscoveryRegistries />} />
            <Route path="/:owner" element={<SkillList />} />
            <Route path="/:owner/:repo" element={<SkillList />} />
            <Route path="/:owner/:repo/:name" element={<SkillDetail />} />
          </Routes>
        </main>
      </div>
    </Router>
  )
}

export default App
