import { useEffect, useMemo, useState } from "react"
import { useParams, useSearchParams, Link } from "react-router-dom"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Badge } from "@/components/ui/badge"
import { Search, Copy, Check, CalendarDays } from "lucide-react"

type Skill = {
  id: number
  name: string
  owner: string
  repo: string
  host: string
  latest_version: string | null
  description: string | null
  created_at: string
  install_count: number
  stars: number
}

type PaginatedSkillsResponse<T> = {
  items: T[]
  total: number
  page: number
  per_page: number
  has_next: boolean
}

const PAGE_SIZE = 20

export default function SkillList() {
  const { host, org, repo } = useParams()
  const [searchParams, setSearchParams] = useSearchParams()
  const [skills, setSkills] = useState<Skill[]>([])
  const [loading, setLoading] = useState(false)
  const [total, setTotal] = useState(0)
  const [hasNext, setHasNext] = useState(false)
  const [sortBy, setSortBy] = useState("created_at")
  const [sortOrder, setSortOrder] = useState("desc")
  const [compatibility, setCompatibility] = useState("")
  const [hasVersion, setHasVersion] = useState<"all" | "yes" | "no">("all")
  const [copiedGlobalCommand, setCopiedGlobalCommand] = useState(false)
  const [copiedSkillId, setCopiedSkillId] = useState<number | null>(null)

  const q = searchParams.get("q") || ""
  const page = Math.max(1, Number.parseInt(searchParams.get("page") || "1", 10) || 1)

  useEffect(() => {
    fetchSkills()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, org, repo, q, page, sortBy, sortOrder, compatibility, hasVersion])

  const title = useMemo(() => {
    if (host && org && repo) return `${host} / ${org} / ${repo}`
    if (host && org) return `${host} / ${org}`
    if (host) return `${host}`
    return "Skill Leaderboard"
  }, [host, org, repo])

  const subtitle = useMemo(() => {
    if (host && org && repo) return "Browse skills in this repository."
    if (host && org) return `Browse skills published by ${org} on ${host}.`
    if (host) return `Browse skills from ${host}.`
    return "Discover and install capabilities for your AI agents."
  }, [host, org, repo])

  const fetchSkills = async () => {
    setLoading(true)
    try {
      const params: Record<string, string | number | boolean> = {
        page,
        per_page: PAGE_SIZE,
        sort_by: sortBy,
        order: sortOrder,
      }
      if (q) params.q = q
      if (host) params.host = host
      if (org) params.org = org
      if (repo) params.repo = repo
      if (compatibility) params.compatibility = compatibility
      if (hasVersion === "yes") params.has_version = true
      if (hasVersion === "no") params.has_version = false

      const res = await api.get<ApiResponse<PaginatedSkillsResponse<Skill>>>("/api/skills", {
        params,
      })
      if (res.data.code === 200 && res.data.data) {
        setSkills(res.data.data.items ?? [])
        setTotal(res.data.data.total ?? 0)
        setHasNext(res.data.data.has_next ?? false)
      } else {
        setSkills([])
        setTotal(0)
        setHasNext(false)
      }
    } catch (error) {
      console.error("Failed to fetch skills", error)
      setSkills([])
      setTotal(0)
      setHasNext(false)
    } finally {
      setLoading(false)
    }
  }

  const updateParams = (updates: Record<string, string>) => {
    const next = new URLSearchParams(searchParams)
    Object.entries(updates).forEach(([key, value]) => {
      if (value) {
        next.set(key, value)
      } else {
        next.delete(key)
      }
    })
    setSearchParams(next)
  }

  const copyText = async (text: string) => {
<<<<<<< HEAD
=======
    // Try modern clipboard API first (only works in secure contexts)
>>>>>>> 249762c (fix(frontend): optimize ui)
    if (window.isSecureContext && navigator.clipboard) {
      try {
        await navigator.clipboard.writeText(text)
        return true
      } catch {
<<<<<<< HEAD
        // fallback below
      }
    }

=======
        // Fall through to fallback method
      }
    }

    // Fallback: use textarea method for non-secure contexts
>>>>>>> 249762c (fix(frontend): optimize ui)
    try {
      const textarea = document.createElement("textarea")
      textarea.value = text
      textarea.style.position = "fixed"
      textarea.style.left = "-999999px"
      textarea.style.top = "-999999px"
      document.body.appendChild(textarea)
      textarea.focus()
      textarea.select()
      const success = document.execCommand("copy")
      textarea.remove()
      return success
    } catch {
      return false
    }
  }

  const onCopyGlobalCommand = async () => {
    const copied = await copyText("npx skills add <owner/repo>")
    if (copied) {
      setCopiedGlobalCommand(true)
      window.setTimeout(() => setCopiedGlobalCommand(false), 1200)
    }
  }

  const onCopyInstallCommand = async (skill: Skill) => {
    const copied = await copyText(`npx skills add ${skill.owner}/${skill.repo} --skill ${skill.name}`)
    if (copied) {
      setCopiedSkillId(skill.id)
      window.setTimeout(() => setCopiedSkillId(null), 1200)
    }
  }

  const skillPath = (skill: Skill) => `/${skill.host}/${skill.owner}/${skill.repo}/skill/${skill.name}`
  const ownerPath = (skill: Skill) => `/${skill.host}/${skill.owner}`
  const repoPath = (skill: Skill) => `/${skill.host}/${skill.owner}/${skill.repo}`

  return (
    <div className="container mx-auto py-8 space-y-6">
      <div className="flex flex-col gap-4">
        <h1 className="text-3xl font-bold tracking-tight">{title}</h1>
        <p className="text-muted-foreground">{subtitle}</p>

        {!host && !org && !repo && (
          <div className="bg-zinc-900 text-zinc-50 p-4 rounded-lg flex items-center justify-between gap-4 font-mono text-sm">
            <span>$ npx skills add &lt;owner/repo&gt;</span>
            <Button
              variant="ghost"
              size="sm"
              className="text-zinc-300 hover:text-white"
              onClick={onCopyGlobalCommand}
              aria-label="Copy global install command"
            >
              {copiedGlobalCommand ? "Copied" : "Copy"}
            </Button>
          </div>
        )}
      </div>

      <div className="flex flex-col gap-3">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search skills..."
            value={q}
            onChange={(e) => updateParams({ q: e.target.value, page: "1" })}
            className="pl-8"
          />
        </div>

        <div className="flex flex-wrap gap-2">
          <select
            value={sortBy}
            onChange={(e) => {
              setSortBy(e.target.value)
              updateParams({ page: "1" })
            }}
            className="h-9 rounded-md border border-input bg-background px-3 text-sm"
          >
            <option value="created_at">Created</option>
            <option value="updated_at">Updated</option>
            <option value="name">Name</option>
            <option value="stars">Stars</option>
            <option value="installs">Installs</option>
          </select>
          <select
            value={sortOrder}
            onChange={(e) => {
              setSortOrder(e.target.value)
              updateParams({ page: "1" })
            }}
            className="h-9 rounded-md border border-input bg-background px-3 text-sm"
          >
            <option value="desc">Descending</option>
            <option value="asc">Ascending</option>
          </select>
          <Input
            value={compatibility}
            onChange={(e) => {
              setCompatibility(e.target.value)
              updateParams({ page: "1" })
            }}
            placeholder="Compatibility tag"
            className="w-44"
          />
          <select
            value={hasVersion}
            onChange={(e) => {
              setHasVersion(e.target.value as "all" | "yes" | "no")
              updateParams({ page: "1" })
            }}
            className="h-9 rounded-md border border-input bg-background px-3 text-sm"
          >
            <option value="all">All Versions</option>
            <option value="yes">Has Version</option>
            <option value="no">No Version</option>
          </select>
        </div>
      </div>

      <div className="text-sm text-muted-foreground">{loading ? "Loading..." : `${total} total skills`}</div>

      <div className="hidden md:block border rounded-md">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[50px]">#</TableHead>
              <TableHead>Skill</TableHead>
              <TableHead className="w-[140px]">Latest Version</TableHead>
              <TableHead className="text-right">Created</TableHead>
              <TableHead className="text-right">Installs</TableHead>
              <TableHead className="w-[140px] text-right">Install</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {loading ? (
              <TableRow>
                <TableCell colSpan={6} className="text-center py-8">
                  Loading skills...
                </TableCell>
              </TableRow>
            ) : skills.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="text-center py-8">
                  No skills found.
                </TableCell>
              </TableRow>
            ) : (
              skills.map((skill, index) => (
                <TableRow key={skill.id} className="align-top">
                  <TableCell className="font-medium text-muted-foreground pt-5">
                    {(page - 1) * PAGE_SIZE + index + 1}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-col gap-1.5">
                      <Link to={skillPath(skill)} className="font-semibold hover:underline text-base">
                        {skill.name}
                      </Link>
                      <span className="text-sm text-muted-foreground line-clamp-2">
                        {skill.description || "No description provided."}
                      </span>
                      <div className="text-xs text-muted-foreground">
                        by <Link to={ownerPath(skill)} className="hover:underline">{skill.owner}</Link>
                        {" / "}
                        <Link to={repoPath(skill)} className="hover:underline">{skill.repo}</Link>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    {skill.latest_version ? (
                      <Badge variant="secondary">{skill.latest_version}</Badge>
                    ) : (
                      <span className="text-sm text-muted-foreground">-</span>
                    )}
                  </TableCell>
                  <TableCell className="text-right text-muted-foreground">
                    {new Date(skill.created_at).toLocaleDateString()}
                  </TableCell>
                  <TableCell className="text-right text-muted-foreground">
                    {skill.install_count ?? 0}
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      variant="outline"
                      size="sm"
                      className="gap-2"
                      onClick={() => onCopyInstallCommand(skill)}
                    >
                      {copiedSkillId === skill.id ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                      <span>{copiedSkillId === skill.id ? "Copied" : "Copy"}</span>
                    </Button>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      <div className="space-y-3 md:hidden">
        {loading ? (
          <div className="rounded-lg border bg-card p-6 text-center text-muted-foreground">
            Loading skills...
          </div>
        ) : skills.length === 0 ? (
          <div className="rounded-lg border bg-card p-6 text-center text-muted-foreground">
            No skills found.
          </div>
        ) : (
          skills.map((skill) => (
            <article key={skill.id} className="rounded-lg border bg-card p-4 space-y-3">
              <div className="space-y-1">
                <Link to={skillPath(skill)} className="text-lg font-semibold hover:underline">
                  {skill.name}
                </Link>
                <p className="text-sm text-muted-foreground">{skill.description || "No description provided."}</p>
              </div>

              <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
                <span className="inline-flex items-center gap-1 whitespace-nowrap">
                  <CalendarDays className="h-3 w-3" />
                  {new Date(skill.created_at).toLocaleDateString()}
                </span>
                <span>Installs: {skill.install_count ?? 0}</span>
                {skill.latest_version && <Badge variant="secondary">{skill.latest_version}</Badge>}
              </div>

              <Button
                variant="outline"
                size="sm"
                className="w-full gap-2"
                onClick={() => onCopyInstallCommand(skill)}
              >
                {copiedSkillId === skill.id ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                {copiedSkillId === skill.id ? "Copied install command" : "Copy install command"}
              </Button>
            </article>
          ))
        )}
      </div>

      <div className="flex items-center justify-end space-x-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => updateParams({ page: String(Math.max(1, page - 1)) })}
          disabled={page <= 1}
        >
          Previous
        </Button>
        <div className="text-sm font-medium">Page {page}</div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => updateParams({ page: String(page + 1) })}
          disabled={!hasNext}
        >
          Next
        </Button>
      </div>
    </div>
  )
}
