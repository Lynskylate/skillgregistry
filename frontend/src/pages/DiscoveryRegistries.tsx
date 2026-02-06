import { FormEvent, useEffect, useState } from "react"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"
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
  platform: string
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

export default function DiscoveryRegistries() {
  const [registries, setRegistries] = useState<DiscoveryRegistry[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const [createToken, setCreateToken] = useState("")
  const [createQueries, setCreateQueries] = useState("topic:agent-skill")
  const [createInterval, setCreateInterval] = useState("3600")

  const [editing, setEditing] = useState<DiscoveryRegistry | null>(null)
  const [editQueries, setEditQueries] = useState("")
  const [editInterval, setEditInterval] = useState("")

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
    const queries = parseQueries(createQueries)

    try {
      const res = await api.post<ApiResponse<DiscoveryRegistry>>("/api/admin/discovery-registries", {
        platform: "github",
        token: createToken,
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
    }
  }

  const startEdit = (row: DiscoveryRegistry) => {
    setEditing(row)
    setEditQueries(row.queries.join("\n"))
    setEditInterval(String(row.schedule_interval_seconds))
  }

  const onSaveEdit = async (e: FormEvent) => {
    e.preventDefault()
    if (!editing) return

    try {
      const res = await api.patch<ApiResponse<DiscoveryRegistry>>(
        `/api/admin/discovery-registries/${editing.id}`,
        {
          queries: parseQueries(editQueries),
          schedule_interval_seconds: Number(editInterval),
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
    }
  }

  const onDelete = async (id: number) => {
    if (!window.confirm("Delete this discovery registry?")) {
      return
    }

    try {
      const res = await api.delete<ApiResponse<{ deleted: boolean }>>(
        `/api/admin/discovery-registries/${id}`,
      )
      if (res.data.code === 200) {
        setActionMessage("Discovery registry deleted")
        await fetchRegistries()
      } else {
        setActionMessage(res.data.message)
      }
    } catch (err: any) {
      setActionMessage(err?.response?.data?.message ?? "Failed to delete discovery registry")
    }
  }

  const onTestHealth = async (id: number) => {
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
    }
  }

  const onTrigger = async (id: number) => {
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
    }
  }

  return (
    <div className="container mx-auto py-8 space-y-6">
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Discovery Registries</h1>
        <p className="text-muted-foreground">
          Manage GitHub discovery sources and trigger discovery workflows.
        </p>
      </div>

      {error && <div className="rounded border border-red-500 bg-red-50 p-3 text-red-700">{error}</div>}
      {actionMessage && (
        <div className="rounded border border-zinc-300 bg-zinc-50 p-3 text-sm">{actionMessage}</div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Create Registry</CardTitle>
        </CardHeader>
        <CardContent>
          <form className="space-y-3" onSubmit={onCreate}>
            <div className="space-y-1">
              <label className="text-sm font-medium">GitHub Token</label>
              <Input
                type="password"
                value={createToken}
                onChange={(e) => setCreateToken(e.target.value)}
                placeholder="ghp_..."
              />
            </div>
            <div className="space-y-1">
              <label className="text-sm font-medium">Queries (comma or newline separated)</label>
              <textarea
                className="min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                value={createQueries}
                onChange={(e) => setCreateQueries(e.target.value)}
              />
            </div>
            <div className="space-y-1">
              <label className="text-sm font-medium">Schedule Interval (seconds)</label>
              <Input
                type="number"
                min={60}
                value={createInterval}
                onChange={(e) => setCreateInterval(e.target.value)}
              />
            </div>
            <Button type="submit">Create Registry</Button>
          </form>
        </CardContent>
      </Card>

      {editing && (
        <Card>
          <CardHeader>
            <CardTitle>Edit Registry #{editing.id}</CardTitle>
          </CardHeader>
          <CardContent>
            <form className="space-y-3" onSubmit={onSaveEdit}>
              <div className="space-y-1">
                <label className="text-sm font-medium">Queries</label>
                <textarea
                  className="min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={editQueries}
                  onChange={(e) => setEditQueries(e.target.value)}
                />
              </div>
              <div className="space-y-1">
                <label className="text-sm font-medium">Schedule Interval (seconds)</label>
                <Input
                  type="number"
                  min={60}
                  value={editInterval}
                  onChange={(e) => setEditInterval(e.target.value)}
                />
              </div>
              <div className="flex gap-2">
                <Button type="submit">Save</Button>
                <Button type="button" variant="outline" onClick={() => setEditing(null)}>
                  Cancel
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      )}

      <div className="border rounded-md">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>ID</TableHead>
              <TableHead>Platform</TableHead>
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
                <TableCell colSpan={7} className="text-center py-6">Loading...</TableCell>
              </TableRow>
            ) : registries.length === 0 ? (
              <TableRow>
                <TableCell colSpan={7} className="text-center py-6">No discovery registries</TableCell>
              </TableRow>
            ) : (
              registries.map((row) => (
                <TableRow key={row.id}>
                  <TableCell>{row.id}</TableCell>
                  <TableCell>{row.platform}</TableCell>
                  <TableCell className="max-w-96 break-words">{row.queries.join(", ")}</TableCell>
                  <TableCell>{row.schedule_interval_seconds}s</TableCell>
                  <TableCell>{row.last_health_status ?? "unknown"}</TableCell>
                  <TableCell>
                    {row.next_run_at ? new Date(row.next_run_at).toLocaleString() : "-"}
                  </TableCell>
                  <TableCell className="text-right space-x-2">
                    <Button variant="outline" size="sm" onClick={() => startEdit(row)}>
                      Edit
                    </Button>
                    <Button variant="outline" size="sm" onClick={() => onTestHealth(row.id)}>
                      Test
                    </Button>
                    <Button variant="outline" size="sm" onClick={() => onTrigger(row.id)}>
                      Trigger
                    </Button>
                    <Button variant="outline" size="sm" onClick={() => onDelete(row.id)}>
                      Delete
                    </Button>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}
