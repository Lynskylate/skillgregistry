import { FormEvent, useState } from "react"
import { Link, useNavigate } from "react-router-dom"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"
import { setAccessToken } from "@/lib/auth"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

type LoginResponse = {
  access_token: string
  token_type: string
  expires_in: number
}

export default function Register() {
  const navigate = useNavigate()
  const [username, setUsername] = useState("")
  const [email, setEmail] = useState("")
  const [displayName, setDisplayName] = useState("")
  const [password, setPassword] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const onRegister = async (e: FormEvent) => {
    e.preventDefault()
    setSubmitting(true)
    setError(null)
    try {
      const res = await api.post<ApiResponse<LoginResponse>>("/api/auth/register", {
        username,
        password,
        email: email || undefined,
        display_name: displayName || undefined,
      })
      const token = res.data.data?.access_token
      if (!token) throw new Error("missing token")
      setAccessToken(token)
      navigate("/")
    } catch (e: any) {
      setError(e?.response?.data?.message ?? "Registration failed.")
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="container mx-auto py-12 max-w-lg">
      <Card>
        <CardHeader>
          <CardTitle>Create account</CardTitle>
        </CardHeader>
        <CardContent className="space-y-6">
          <form className="space-y-3" onSubmit={onRegister}>
            <div className="space-y-2">
              <label htmlFor="register-username" className="text-sm text-muted-foreground">Username</label>
              <Input
                id="register-username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoComplete="username"
                required
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="register-email" className="text-sm text-muted-foreground">Email (optional)</label>
              <Input
                id="register-email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                autoComplete="email"
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="register-display-name" className="text-sm text-muted-foreground">Display name (optional)</label>
              <Input
                id="register-display-name"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="register-password" className="text-sm text-muted-foreground">Password</label>
              <Input
                id="register-password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="new-password"
                required
              />
            </div>
            {error && <div className="text-sm text-destructive">{error}</div>}
            <Button type="submit" className="w-full" disabled={submitting}>
              Create account
            </Button>
            <div className="text-sm text-muted-foreground">
              Already have an account?{" "}
              <Link to="/login" className="underline">
                Sign in
              </Link>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  )
}
