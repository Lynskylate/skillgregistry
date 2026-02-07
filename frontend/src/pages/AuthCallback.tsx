import { useEffect } from "react"
import { useNavigate } from "react-router-dom"
import { useAuth } from "@/contexts/AuthContext"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"

type LoginResponse = {
  access_token: string
  token_type: string
  expires_in: number
}

export default function AuthCallback() {
  const navigate = useNavigate()
  const { applyAccessToken } = useAuth()

  useEffect(() => {
    const run = async () => {
      try {
        const res = await api.post<ApiResponse<LoginResponse>>("/api/auth/refresh")
        const token = res.data.data?.access_token
        if (token) {
          await applyAccessToken(token)
          navigate("/", { replace: true })
        } else {
          navigate("/login", { replace: true })
        }
      } catch {
        navigate("/login", { replace: true })
      }
    }
    run()
  }, [applyAccessToken, navigate])

  return <div className="p-8 text-center">Signing you in...</div>
}
