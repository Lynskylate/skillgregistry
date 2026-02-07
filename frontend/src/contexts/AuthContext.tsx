import { createContext, useContext, useEffect, useState, type ReactNode } from "react"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"

type MeResponse = {
  user_id: string
  username: string | null
  role: string
  display_name: string | null
  primary_email: string | null
}

type AuthContextType = {
  role: string | null
  loading: boolean
  user: MeResponse | null
  refreshAuth: () => Promise<void>
}

const AuthContext = createContext<AuthContextType | undefined>(undefined)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [role, setRole] = useState<string | null>(null)
  const [user, setUser] = useState<MeResponse | null>(null)
  const [loading, setLoading] = useState(true)

  const fetchMe = async () => {
    try {
      const res = await api.get<ApiResponse<MeResponse>>("/api/me")
      if (res.data.code === 200) {
        setRole(res.data.data?.role ?? null)
        setUser(res.data.data ?? null)
      } else {
        setRole(null)
        setUser(null)
      }
    } catch {
      setRole(null)
      setUser(null)
    } finally {
      setLoading(false)
    }
  }

  const refreshAuth = async () => {
    setLoading(true)
    await fetchMe()
  }

  useEffect(() => {
    fetchMe()
  }, [])

  return (
    <AuthContext.Provider value={{ role, loading, user, refreshAuth }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const context = useContext(AuthContext)
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider")
  }
  return context
}
