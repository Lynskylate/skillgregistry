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
  last_run_status: string | null
  last_run_message: string | null
  next_run_at: string | null
  created_at: string
  updated_at: string
}

type HealthResult = {
  ok: boolean
  message: string
  checked_at: string
  started_at?: string | null
}

type TriggerResult = {
  ok: boolean
  message: string
  workflow_id: string
  started_at: string
}

type ValidateDeleteResponse = {
  can_delete: boolean
  reasons: string[]
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
  const [actionMessage, setActionMessage] = useState<string | null>(null)

  const [deleteValidation, setDeleteValidation] = useState<{ id: number; reasons: string[] } | null>(null)
  const [deleteConfirmationInput, setDeleteConfirmationInput] = useState("")

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

  const validateDelete = async (id: number) => {
    setBusyRowId(id)
    try {
      const res = await api.post<ApiResponse<ValidateDeleteResponse>>(
        `/api/admin/discovery-registries/${id}/validate-delete`,
      )
      if (res.data.code === 200 && res.data.data) {
        setDeleteValidation({ id, reasons: res.data.data.reasons ?? [] })
        setDeleteConfirmationInput("")
      } else {
        setActionMessage(res.data.message)
      }
    } catch (err: any) {
      setActionMessage(err?.response?.data?.message ?? "Validation failed")
    } finally {
      setBusyRowId(null)
    }
  }

  const onDelete = async (id: number) => {
    if (deleteValidation && deleteConfirmationInput !== deleteValidation.id.toString()) {
      setActionMessage("Please type the correct registry ID to confirm")
      return
    }

    setBusyRowId(id)
    try {
      const res = await api.delete<ApiResponse<{ deleted: boolean }>>(
        `/api/admin/discovery-registries/${id}`,
        { data: { confirmation_id: deleteConfirmationInput } },
      )
      if (res.data.code === 200) {
        setActionMessage("Discovery registry deleted")
        setDeleteValidation(null)
        setDeleteConfirmationInput("")
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
    try {
      const res = await api.post<ApiResponse<TriggerResult>>(
        `/api/admin/discovery-registries/${id}/trigger`,
      )
      if (res.data.code === 200) {
        const result = res.data.data
        setActionMessage(result ? `${result.message}: ${result.workflow_id}` : "Discovery trigger accepted")
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
          <p className="text-xs uppercase text-muted-foreground">Needs Attention</p>
          <p className="mt-2 text-2xl font-semibold text-amber-600">{attentionSources}</p>
        </div>
      </div>

      <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
        <Card>
          <CardHeader>
            <CardTitle>Add Discovery Source</CardTitle>
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
                  required
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
              <TableHead>Run Status</TableHead>
              <TableHead>Run Message</TableHead>
              <TableHead>Started At</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {loading ? (
              <TableRow>
                <TableCell colSpan={10} className="text-center py-6">Loading...</TableCell>
              </TableRow>
            ) : registries.length === 0 ? (
              <TableRow>
                <TableCell colSpan={10} className="text-center py-6">No discovery registries</TableCell>
              </TableRow>
            ) : (
              registries.map((row) => {
                const rowBusy = busyRowId === row.id
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
                    <TableCell>
                      {row.last_run_status ? (
                        <Badge variant={row.last_run_status === "success" ? "default" : "destructive"}>
                          {row.last_run_status}
                        </Badge>
                      ) : (
                        <Badge variant="outline">Never run</Badge>
                      )}
                    </TableCell>
                    <TableCell className="max-w-72 truncate" title={row.last_run_message ?? undefined}>
                      {row.last_run_message ?? "-"}
                    </TableCell>
                    <TableCell>{formatDateTime(row.last_run_at)}</TableCell>
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
                        <Button
                          variant="destructive"
                          size="sm"
                          onClick={() => validateDelete(row.id)}
                          disabled={rowBusy}
                        >
                          Delete
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                )
              })
            )}
          </TableBody>
        </Table>
      </div>

      {deleteValidation && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
          <Card className="w-full max-w-md">
            <CardHeader>
              <CardTitle>Confirm Deletion</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {deleteValidation.reasons.length > 0 && (
                <div className="space-y-2">
                  <p className="font-medium">Please note:</p>
                  <ul className="list-disc list-inside space-y-1 text-sm text-muted-foreground">
                    {deleteValidation.reasons.map((reason, index) => (
                      <li key={`${reason}-${index}`}>{reason}</li>
                    ))}
                  </ul>
                </div>
              )}
              <div>
                <label className="text-sm font-medium">Type registry ID to confirm:</label>
                <Input
                  value={deleteConfirmationInput}
                  onChange={(e) => setDeleteConfirmationInput(e.target.value)}
                  placeholder={deleteValidation.id.toString()}
                />
              </div>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={() => setDeleteValidation(null)}>
                  Cancel
                </Button>
                <Button
                  variant="destructive"
                  onClick={() => onDelete(deleteValidation.id)}
                  disabled={deleteConfirmationInput !== deleteValidation.id.toString()}
                >
                  Delete Registry
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  )
}
