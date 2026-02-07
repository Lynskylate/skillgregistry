import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react"
import { api } from "@/lib/api"
import { setAccessToken } from "@/lib/auth"
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
  applyAccessToken: (token: string) => Promise<void>
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextType | undefined>(undefined)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [role, setRole] = useState<string | null>(null)
  const [user, setUser] = useState<MeResponse | null>(null)
  const [loading, setLoading] = useState(true)

  const setSignedOutState = useCallback(() => {
    setRole(null)
    setUser(null)
    setLoading(false)
  }, [])

  const clearSession = useCallback(() => {
    setAccessToken(null)
    setSignedOutState()
  }, [setSignedOutState])

  const fetchMe = useCallback(async () => {
    try {
      const res = await api.get<ApiResponse<MeResponse>>("/api/me")
      if (res.data.code === 200) {
        setRole(res.data.data?.role ?? null)
        setUser(res.data.data ?? null)
      } else {
        setSignedOutState()
      }
    } catch {
      setSignedOutState()
    } finally {
      setLoading(false)
    }
  }, [setSignedOutState])

  const refreshAuth = useCallback(async () => {
    setLoading(true)
    await fetchMe()
  }, [fetchMe])

  const applyAccessToken = useCallback(async (token: string) => {
    setAccessToken(token)
    await refreshAuth()
  }, [refreshAuth])

  const logout = useCallback(async () => {
    try {
      await api.post<ApiResponse<null>>("/api/auth/logout")
    } catch {
      // Always clear local auth state even if server-side revoke fails.
    } finally {
      clearSession()
    }
  }, [clearSession])

  useEffect(() => {
    fetchMe()
  }, [fetchMe])

  return (
    <AuthContext.Provider value={{ role, loading, user, refreshAuth, applyAccessToken, logout }}>
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
