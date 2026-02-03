import axios from "axios"
import type { ApiResponse } from "@/lib/types"
import { getAccessToken, setAccessToken } from "@/lib/auth"

type LoginResponse = {
  access_token: string
  token_type: string
  expires_in: number
}

type AxiosRequestConfigWithRetry = Parameters<
  (typeof axios)["interceptors"]["response"]["use"]
>[1] extends (arg: any) => any
  ? Parameters<Parameters<(typeof axios)["interceptors"]["response"]["use"]>[1]>[0]["config"] & {
      _retry?: boolean
    }
  : any

export const api = axios.create({
  withCredentials: true,
})

api.interceptors.request.use((config) => {
  const token = getAccessToken()
  if (token) {
    config.headers = config.headers ?? {}
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

api.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error?.config as AxiosRequestConfigWithRetry | undefined
    if (!original) return Promise.reject(error)

    if (error?.response?.status === 401 && !original._retry) {
      original._retry = true
      try {
        const refreshRes = await api.post<ApiResponse<LoginResponse>>("/api/auth/refresh")
        const newToken = refreshRes.data.data?.access_token
        if (newToken) {
          setAccessToken(newToken)
          original.headers = original.headers ?? {}
          original.headers.Authorization = `Bearer ${newToken}`
          return api(original)
        }
      } catch {
        setAccessToken(null)
      }
    }

    return Promise.reject(error)
  }
)

