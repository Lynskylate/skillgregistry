import { useEffect } from "react"
import { useNavigate } from "react-router-dom"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"
import { setAccessToken } from "@/lib/auth"

type LoginResponse = {
  access_token: string
  token_type: string
  expires_in: number
}

export default function AuthCallback() {
  const navigate = useNavigate()

  useEffect(() => {
    const run = async () => {
      try {
        const res = await api.post<ApiResponse<LoginResponse>>("/api/auth/refresh")
        const token = res.data.data?.access_token
        if (token) {
          setAccessToken(token)
          navigate("/", { replace: true })
        } else {
          navigate("/login", { replace: true })
        }
      } catch {
        navigate("/login", { replace: true })
      }
    }
    run()
  }, [navigate])

  return <div className="p-8 text-center">Signing you in...</div>
}

