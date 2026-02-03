import { useState } from "react"
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

  const onRegister = async () => {
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
          <div className="space-y-3">
            <div className="space-y-2">
              <div className="text-sm text-muted-foreground">Username</div>
              <Input value={username} onChange={(e) => setUsername(e.target.value)} />
            </div>
            <div className="space-y-2">
              <div className="text-sm text-muted-foreground">Email (optional)</div>
              <Input value={email} onChange={(e) => setEmail(e.target.value)} />
            </div>
            <div className="space-y-2">
              <div className="text-sm text-muted-foreground">Display name (optional)</div>
              <Input value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
            </div>
            <div className="space-y-2">
              <div className="text-sm text-muted-foreground">Password</div>
              <Input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
            {error && <div className="text-sm text-destructive">{error}</div>}
            <Button className="w-full" disabled={submitting} onClick={onRegister}>
              Create account
            </Button>
            <div className="text-sm text-muted-foreground">
              Already have an account?{" "}
              <Link to="/login" className="underline">
                Sign in
              </Link>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
