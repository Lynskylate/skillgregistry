import { FormEvent, useEffect, useState } from "react"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"

type DiscoveryRegistry = {
  id: number
  provider: string
  url: string
  queries: string[]
  schedule_interval_seconds: number
  token_configured: boolean
  last_health_status: string | null
  last_health_message: string | null
  last_health_checked_at: string | null
  last_run_at: string | null
  next_run_at: string | null
  created_at: string
  updated_at: string
}

type HealthResult = {
  ok: boolean
  message: string
  checked_at: string
}

type TriggerResult = {
  workflow_id: string
}

function formatDateTime(value: string | null) {
  if (!value) return "-"
  return new Date(value).toLocaleString()
}

function normalizeHealthStatus(status: string | null) {
  return status?.toLowerCase() ?? "unknown"
}

function isHealthyStatus(status: string | null) {
  const normalized = normalizeHealthStatus(status)
  return normalized === "ok" || normalized === "healthy"
}

function getHealthBadgeClasses(status: string | null) {
  const normalized = normalizeHealthStatus(status)
  if (normalized === "ok" || normalized === "healthy") {
    return "border-emerald-200 bg-emerald-50 text-emerald-700"
  }
  if (["error", "failed", "unhealthy", "down"].includes(normalized)) {
    return "border-red-200 bg-red-50 text-red-700"
  }
  return "border-amber-200 bg-amber-50 text-amber-700"
}

export default function DiscoveryRegistries() {
  const [registries, setRegistries] = useState<DiscoveryRegistry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [createToken, setCreateToken] = useState("")
  const [createUrl, setCreateUrl] = useState("https://api.github.com")
  const [createQueries, setCreateQueries] = useState("topic:agent-skill")
  const [createInterval, setCreateInterval] = useState("3600")

  const [editing, setEditing] = useState<DiscoveryRegistry | null>(null)
  const [editQueries, setEditQueries] = useState("")
  const [editInterval, setEditInterval] = useState("")
  const [editUrl, setEditUrl] = useState("")

  const [creating, setCreating] = useState(false)
  const [saving, setSaving] = useState(false)
  const [busyRowId, setBusyRowId] = useState<number | null>(null)
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<number | null>(null)
  const [actionMessage, setActionMessage] = useState<string | null>(null)

  useEffect(() => {
    fetchRegistries()
  }, [])

  const fetchRegistries = async () => {
    setLoading(true)
    setError(null)
    try {
      const res = await api.get<ApiResponse<DiscoveryRegistry[]>>("/api/admin/discovery-registries")
      if (res.data.code === 200) {
        setRegistries(res.data.data ?? [])
      } else {
        setError(res.data.message)
      }
    } catch (err: any) {
      setError(err?.response?.data?.message ?? "Failed to load discovery registries")
    } finally {
      setLoading(false)
    }
  }

  const parseQueries = (raw: string): string[] => {
    return raw
      .split(/\n|,/)
      .map((q) => q.trim())
      .filter((q) => q.length > 0)
  }

  const onCreate = async (e: FormEvent) => {
    e.preventDefault()
    setActionMessage(null)
    setCreating(true)
    const queries = parseQueries(createQueries)

    try {
      const res = await api.post<ApiResponse<DiscoveryRegistry>>("/api/admin/discovery-registries", {
        provider: "github",
        token: createToken,
        url: createUrl,
        queries,
        schedule_interval_seconds: Number(createInterval),
      })

      if (res.data.code === 200) {
        setCreateToken("")
        setActionMessage("Discovery registry created")
        await fetchRegistries()
      } else {
        setActionMessage(res.data.message)
      }
    } catch (err: any) {
      setActionMessage(err?.response?.data?.message ?? "Failed to create discovery registry")
    } finally {
      setCreating(false)
    }
  }

  const startEdit = (row: DiscoveryRegistry) => {
    setConfirmingDeleteId(null)
    setEditing(row)
    setEditUrl(row.url)
    setEditQueries(row.queries.join("\n"))
    setEditInterval(String(row.schedule_interval_seconds))
  }

  const onSaveEdit = async (e: FormEvent) => {
    e.preventDefault()
    if (!editing) return
    setSaving(true)

    try {
      const res = await api.patch<ApiResponse<DiscoveryRegistry>>(
        `/api/admin/discovery-registries/${editing.id}`,
        {
          queries: parseQueries(editQueries),
          schedule_interval_seconds: Number(editInterval),
          url: editUrl,
        },
      )
      if (res.data.code === 200) {
        setActionMessage("Discovery registry updated")
        setEditing(null)
        await fetchRegistries()
      } else {
        setActionMessage(res.data.message)
      }
    } catch (err: any) {
      setActionMessage(err?.response?.data?.message ?? "Failed to update discovery registry")
    } finally {
      setSaving(false)
    }
  }

  const onDelete = async (id: number) => {
    setBusyRowId(id)
    try {
      const res = await api.delete<ApiResponse<{ deleted: boolean }>>(
        `/api/admin/discovery-registries/${id}`,
      )
      if (res.data.code === 200) {
        setActionMessage("Discovery registry deleted")
        setConfirmingDeleteId(null)
        await fetchRegistries()
      } else {
        setActionMessage(res.data.message)
      }
    } catch (err: any) {
      setActionMessage(err?.response?.data?.message ?? "Failed to delete discovery registry")
    } finally {
      setBusyRowId(null)
    }
  }

  const onTestHealth = async (id: number) => {
    setBusyRowId(id)
    setConfirmingDeleteId(null)
    try {
      const res = await api.post<ApiResponse<HealthResult>>(
        `/api/admin/discovery-registries/${id}/test-health`,
      )
      if (res.data.code === 200) {
        const result = res.data.data
        setActionMessage(result ? `Health check: ${result.message}` : "Health check completed")
        await fetchRegistries()
      } else {
        setActionMessage(res.data.message)
      }
    } catch (err: any) {
      setActionMessage(err?.response?.data?.message ?? "Failed to test health")
    } finally {
      setBusyRowId(null)
    }
  }

  const onTrigger = async (id: number) => {
    setBusyRowId(id)
    setConfirmingDeleteId(null)
    try {
      const res = await api.post<ApiResponse<TriggerResult>>(
        `/api/admin/discovery-registries/${id}/trigger`,
      )
      if (res.data.code === 200) {
        const workflowId = res.data.data?.workflow_id
        setActionMessage(
          workflowId ? `Triggered workflow: ${workflowId}` : "Discovery trigger accepted",
        )
      } else {
        setActionMessage(res.data.message)
      }
    } catch (err: any) {
      setActionMessage(err?.response?.data?.message ?? "Failed to trigger discovery registry")
    } finally {
      setBusyRowId(null)
    }
  }

  const actionIsError = actionMessage ? /(fail|error|denied|invalid)/i.test(actionMessage) : false
  const healthySources = registries.filter((registry) => isHealthyStatus(registry.last_health_status)).length
  const attentionSources = registries.length - healthySources

  return (
    <div className="container mx-auto py-8 space-y-6">
      <div className="space-y-2">
        <h1 className="text-3xl font-bold tracking-tight">Discovery Registries</h1>
        <p className="text-muted-foreground">
          Manage GitHub discovery sources and trigger discovery workflows.
        </p>
      </div>

      {error && <div className="rounded border border-red-500 bg-red-50 p-3 text-red-700">{error}</div>}
      {actionMessage && (
        <div
          className={`rounded p-3 text-sm ${
            actionIsError
              ? "border border-red-500 bg-red-50 text-red-700"
              : "border border-emerald-300 bg-emerald-50 text-emerald-700"
          }`}
        >
          {actionMessage}
        </div>
      )}

      <div className="grid gap-3 sm:grid-cols-3">
        <div className="rounded-lg border bg-card p-4">
          <p className="text-xs uppercase text-muted-foreground">Total Sources</p>
          <p className="mt-2 text-2xl font-semibold">{registries.length}</p>
        </div>
        <div className="rounded-lg border bg-card p-4">
          <p className="text-xs uppercase text-muted-foreground">Healthy</p>
          <p className="mt-2 text-2xl font-semibold text-emerald-600">{healthySources}</p>
        </div>
        <div className="rounded-lg border bg-card p-4">
          <p className="text-xs uppercase text-muted-foreground">Need Attention</p>
          <p className="mt-2 text-2xl font-semibold text-amber-600">{attentionSources}</p>
        </div>
      </div>

      <div className="grid gap-6 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Create Registry</CardTitle>
          </CardHeader>
          <CardContent>
            <form className="space-y-3" onSubmit={onCreate}>
              <div className="space-y-1">
                <label htmlFor="create-token" className="text-sm font-medium">GitHub Token</label>
                <Input
                  id="create-token"
                  type="password"
                  value={createToken}
                  onChange={(e) => setCreateToken(e.target.value)}
                  placeholder="ghp_..."
                />
              </div>
              <div className="space-y-1">
                <label htmlFor="create-api-url" className="text-sm font-medium">GitHub API URL</label>
                <Input
                  id="create-api-url"
                  value={createUrl}
                  onChange={(e) => setCreateUrl(e.target.value)}
                  placeholder="https://api.github.com"
                />
              </div>
              <div className="space-y-1">
                <label htmlFor="create-queries" className="text-sm font-medium">Queries (comma or newline separated)</label>
                <textarea
                  id="create-queries"
                  className="min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={createQueries}
                  onChange={(e) => setCreateQueries(e.target.value)}
                />
              </div>
              <div className="space-y-1">
                <label htmlFor="create-interval" className="text-sm font-medium">Schedule Interval (seconds)</label>
                <Input
                  id="create-interval"
                  type="number"
                  min={60}
                  value={createInterval}
                  onChange={(e) => setCreateInterval(e.target.value)}
                />
              </div>
              <Button type="submit" disabled={creating}>{creating ? "Creating..." : "Create Registry"}</Button>
            </form>
          </CardContent>
        </Card>

        {editing ? (
          <Card>
            <CardHeader>
              <CardTitle>Edit Registry #{editing.id}</CardTitle>
            </CardHeader>
            <CardContent>
              <form className="space-y-3" onSubmit={onSaveEdit}>
                <div className="space-y-1">
                  <label htmlFor="edit-api-url" className="text-sm font-medium">GitHub API URL</label>
                  <Input
                    id="edit-api-url"
                    value={editUrl}
                    onChange={(e) => setEditUrl(e.target.value)}
                    placeholder="https://api.github.com"
                  />
                </div>
                <div className="space-y-1">
                  <label htmlFor="edit-queries" className="text-sm font-medium">Queries</label>
                  <textarea
                    id="edit-queries"
                    className="min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                    value={editQueries}
                    onChange={(e) => setEditQueries(e.target.value)}
                  />
                </div>
                <div className="space-y-1">
                  <label htmlFor="edit-interval" className="text-sm font-medium">Schedule Interval (seconds)</label>
                  <Input
                    id="edit-interval"
                    type="number"
                    min={60}
                    value={editInterval}
                    onChange={(e) => setEditInterval(e.target.value)}
                  />
                </div>
                <div className="flex gap-2">
                  <Button type="submit" disabled={saving}>{saving ? "Saving..." : "Save"}</Button>
                  <Button type="button" variant="outline" onClick={() => setEditing(null)}>
                    Cancel
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        ) : (
          <Card>
            <CardHeader>
              <CardTitle>Tips</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm text-muted-foreground">
              <p>Use one query per line to make each discovery source easy to inspect.</p>
              <p>Start with a 3600-second interval, then tune based on API limits.</p>
              <p>Run a health check after every edit to validate credentials and endpoint access.</p>
            </CardContent>
          </Card>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">Registered Sources</h2>
          <Badge variant="outline">{registries.length} total</Badge>
        </div>
        <p className="text-xs text-muted-foreground">On smaller screens, cards are shown for quick scanning.</p>
      </div>

      <div className="space-y-3 md:hidden">
        {loading ? (
          Array.from({ length: 3 }).map((_, index) => (
            <div key={index} className="rounded-lg border bg-card p-4 space-y-3">
              <div className="h-4 w-32 animate-pulse rounded bg-muted" />
              <div className="h-3 w-full animate-pulse rounded bg-muted" />
              <div className="h-3 w-3/4 animate-pulse rounded bg-muted" />
            </div>
          ))
        ) : registries.length === 0 ? (
          <div className="rounded-lg border bg-card p-6 text-center text-muted-foreground">
            No discovery registries.
          </div>
        ) : (
          registries.map((row) => {
            const rowBusy = busyRowId === row.id
            const confirmingDelete = confirmingDeleteId === row.id
            return (
              <article key={row.id} className="rounded-lg border bg-card p-4 space-y-3">
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <p className="font-medium">#{row.id} {row.provider}</p>
                    <p className="text-xs text-muted-foreground break-all">{row.url}</p>
                  </div>
                  <Badge
                    variant="outline"
                    className={getHealthBadgeClasses(row.last_health_status)}
                    title={row.last_health_message ?? undefined}
                  >
                    {row.last_health_status ?? "unknown"}
                  </Badge>
                </div>

                <div className="space-y-1 text-xs text-muted-foreground">
                  <p><span className="font-medium text-foreground">Queries:</span> {row.queries.join(", ")}</p>
                  <p><span className="font-medium text-foreground">Interval:</span> {row.schedule_interval_seconds}s</p>
                  <p><span className="font-medium text-foreground">Next Run:</span> {formatDateTime(row.next_run_at)}</p>
                  <p><span className="font-medium text-foreground">Token:</span> {row.token_configured ? "Configured" : "Missing"}</p>
                </div>

                <div className="flex flex-wrap gap-2">
                  <Button variant="outline" size="sm" onClick={() => startEdit(row)} disabled={rowBusy}>
                    Edit
                  </Button>
                  <Button variant="outline" size="sm" onClick={() => onTestHealth(row.id)} disabled={rowBusy}>
                    Test
                  </Button>
                  <Button size="sm" onClick={() => onTrigger(row.id)} disabled={rowBusy}>
                    Trigger
                  </Button>
                  {confirmingDelete ? (
                    <>
                      <Button variant="destructive" size="sm" onClick={() => onDelete(row.id)} disabled={rowBusy}>
                        Confirm Delete
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setConfirmingDeleteId(null)}
                        disabled={rowBusy}
                      >
                        Cancel
                      </Button>
                    </>
                  ) : (
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={() => setConfirmingDeleteId(row.id)}
                      disabled={rowBusy}
                    >
                      Delete
                    </Button>
                  )}
                </div>
              </article>
            )
          })
        )}
      </div>

      <div className="hidden md:block border rounded-md">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>ID</TableHead>
              <TableHead>Provider</TableHead>
              <TableHead>URL</TableHead>
              <TableHead>Queries</TableHead>
              <TableHead>Interval</TableHead>
              <TableHead>Health</TableHead>
              <TableHead>Next Run</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {loading ? (
              <TableRow>
                <TableCell colSpan={8} className="text-center py-6">Loading...</TableCell>
              </TableRow>
            ) : registries.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} className="text-center py-6">No discovery registries</TableCell>
              </TableRow>
            ) : (
              registries.map((row) => {
                const rowBusy = busyRowId === row.id
                const confirmingDelete = confirmingDeleteId === row.id
                return (
                  <TableRow key={row.id}>
                    <TableCell>{row.id}</TableCell>
                    <TableCell className="capitalize">{row.provider}</TableCell>
                    <TableCell className="max-w-80 break-words">{row.url}</TableCell>
                    <TableCell className="max-w-96 break-words">{row.queries.join(", ")}</TableCell>
                    <TableCell>{row.schedule_interval_seconds}s</TableCell>
                    <TableCell>
                      <Badge
                        variant="outline"
                        className={getHealthBadgeClasses(row.last_health_status)}
                        title={row.last_health_message ?? undefined}
                      >
                        {row.last_health_status ?? "unknown"}
                      </Badge>
                    </TableCell>
                    <TableCell>{formatDateTime(row.next_run_at)}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex flex-wrap justify-end gap-2">
                        <Button variant="outline" size="sm" onClick={() => startEdit(row)} disabled={rowBusy}>
                          Edit
                        </Button>
                        <Button variant="outline" size="sm" onClick={() => onTestHealth(row.id)} disabled={rowBusy}>
                          Test
                        </Button>
                        <Button size="sm" onClick={() => onTrigger(row.id)} disabled={rowBusy}>
                          Trigger
                        </Button>
                        {confirmingDelete ? (
                          <>
                            <Button variant="destructive" size="sm" onClick={() => onDelete(row.id)} disabled={rowBusy}>
                              Confirm Delete
                            </Button>
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => setConfirmingDeleteId(null)}
                              disabled={rowBusy}
                            >
                              Cancel
                            </Button>
                          </>
                        ) : (
                          <Button
                            variant="destructive"
                            size="sm"
                            onClick={() => setConfirmingDeleteId(row.id)}
                            disabled={rowBusy}
                          >
                            Delete
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                )
              })
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}
