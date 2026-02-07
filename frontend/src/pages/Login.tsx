import { FormEvent, useMemo, useState } from "react"
import { Link, useNavigate, useSearchParams } from "react-router-dom"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"
import { setAccessToken } from "@/lib/auth"
import { useAuth } from "@/contexts/AuthContext"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

type LoginResponse = {
  access_token: string
  token_type: string
  expires_in: number
}

type SsoLookupItem = {
  connection_id: string
  org_id: string
  protocol: string
}

export default function Login() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const { refreshAuth } = useAuth()
  const [identifier, setIdentifier] = useState("")
  const [password, setPassword] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [ssoEmail, setSsoEmail] = useState("")
  const [ssoItems, setSsoItems] = useState<SsoLookupItem[] | null>(null)
  const [ssoLoading, setSsoLoading] = useState(false)
  const redirectPath = searchParams.get("redirect") || "/"
  const requiresAdmin = searchParams.get("reason") === "admin_required"

  const ssoError = useMemo(() => {
    if (ssoItems && ssoItems.length === 0) return "SSO is not configured for this email domain."
    return null
  }, [ssoItems])

  const onLogin = async (e: FormEvent) => {
    e.preventDefault()
    setSubmitting(true)
    setError(null)
    try {
      const res = await api.post<ApiResponse<LoginResponse>>("/api/auth/login", {
        identifier,
        password,
      })
      const token = res.data.data?.access_token
      if (!token) throw new Error("missing token")
      setAccessToken(token)
      await refreshAuth()
      navigate(redirectPath)
    } catch (e: any) {
      setError(e?.response?.data?.message ?? "Sign-in failed.")
    } finally {
      setSubmitting(false)
    }
  }

  const onSsoLookup = async () => {
    setSsoLoading(true)
    setSsoItems(null)
    try {
      const res = await api.post<ApiResponse<SsoLookupItem[]>>("/api/auth/sso/lookup", {
        email: ssoEmail,
      })
      setSsoItems(res.data.data ?? [])
    } catch {
      setSsoItems([])
    } finally {
      setSsoLoading(false)
    }
  }

  const startSso = (connectionId: string) => {
    window.location.href = `/api/auth/sso/${connectionId}/start`
  }

  const startOAuth = (provider: "github" | "google") => {
    window.location.href = `/api/auth/oauth/${provider}/start`
  }

  return (
    <div className="container mx-auto py-12 max-w-lg">
      <Card>
        <CardHeader>
          <CardTitle>Sign in</CardTitle>
        </CardHeader>
        <CardContent className="space-y-6">
          {requiresAdmin && (
            <div className="rounded border border-zinc-300 bg-zinc-50 p-3 text-sm">
              Admin access is required to open that page.
            </div>
          )}
          <form className="space-y-3" onSubmit={onLogin}>
            <div className="space-y-2">
              <label htmlFor="identifier" className="text-sm text-muted-foreground">Username / Email</label>
              <Input
                id="identifier"
                value={identifier}
                onChange={(e) => setIdentifier(e.target.value)}
                autoComplete="username"
                required
              />
            </div>
            <div className="space-y-2">
              <label htmlFor="password" className="text-sm text-muted-foreground">Password</label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
                required
              />
            </div>
            {error && <div className="text-sm text-destructive">{error}</div>}
            <Button type="submit" className="w-full" disabled={submitting}>
              Sign in
            </Button>
            <div className="text-sm text-muted-foreground">
              Don&apos;t have an account?{" "}
              <Link to="/register" className="underline">
                Create one
              </Link>
            </div>
          </form>

          <div className="space-y-3">
            <div className="text-sm text-muted-foreground">Continue with</div>
            <div className="grid grid-cols-2 gap-2">
              <Button variant="outline" onClick={() => startOAuth("github")}>
                GitHub
              </Button>
              <Button variant="outline" onClick={() => startOAuth("google")}>
                Google
              </Button>
            </div>
          </div>

          <div className="space-y-3">
            <label htmlFor="sso-email" className="text-sm text-muted-foreground">Enterprise SSO</label>
            <div className="flex gap-2">
              <Input
                id="sso-email"
                placeholder="Enter your email to find your organization's SSO"
                value={ssoEmail}
                onChange={(e) => setSsoEmail(e.target.value)}
                autoComplete="email"
              />
              <Button variant="outline" disabled={ssoLoading} onClick={onSsoLookup}>
                Look up
              </Button>
            </div>
            {ssoError && <div className="text-sm text-muted-foreground">{ssoError}</div>}
            {ssoItems && ssoItems.length > 0 && (
              <div className="space-y-2">
                {ssoItems.map((item) => (
                  <Button
                    key={item.connection_id}
                    variant="outline"
                    className="w-full justify-between"
                    onClick={() => startSso(item.connection_id)}
                  >
                    <span>Connection {item.connection_id}</span>
                    <span className="text-muted-foreground text-xs">{item.protocol}</span>
                  </Button>
                ))}
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
