import { useEffect, useState } from "react"
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
import { Search, Filter, SortAsc, Copy, Check, CalendarDays } from "lucide-react"

interface Skill {
  id: number
  name: string
  owner: string
  repo: string
  latest_version: string | null
  description: string | null
  created_at: string
}

export default function SkillList() {
  const { owner, repo } = useParams()
  const [searchParams, setSearchParams] = useSearchParams()
  const [skills, setSkills] = useState<Skill[]>([])
  const [loading, setLoading] = useState(false)
  const [copiedGlobalCommand, setCopiedGlobalCommand] = useState(false)
  const [copiedSkillId, setCopiedSkillId] = useState<number | null>(null)

  const q = searchParams.get("q") || ""
  const page = parseInt(searchParams.get("page") || "1", 10)

  useEffect(() => {
    fetchSkills()
  }, [owner, repo, q, page])

  const fetchSkills = async () => {
    setLoading(true)
    try {
      const params: Record<string, string | number> = { page, per_page: 20 }
      if (q) params.q = q
      if (owner) params.owner = owner
      if (repo) params.repo = repo

      const res = await api.get<ApiResponse<Skill[]>>("/api/skills", { params })
      if (res.data.code === 200) {
        setSkills(res.data.data ?? [])
      }
    } catch (error) {
      console.error("Failed to fetch skills", error)
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
    try {
      await navigator.clipboard.writeText(text)
      return true
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
    const copied = await copyText(`npx skills add ${skill.owner}/${skill.repo}/${skill.name}`)
    if (copied) {
      setCopiedSkillId(skill.id)
      window.setTimeout(() => setCopiedSkillId(null), 1200)
    }
  }

  return (
    <div className="container mx-auto py-8 space-y-6">
      <div className="flex flex-col gap-4">
        <h1 className="text-3xl font-bold tracking-tight">
          {repo ? `${owner} / ${repo}` : owner ? `${owner} Skills` : "Skill Leaderboard"}
        </h1>
        <p className="text-muted-foreground">
          {repo
            ? "Browse skills in this repository."
            : owner
              ? `Browse skills published by ${owner}.`
              : "Discover and install capabilities for your AI agents."}
        </p>

        {!owner && !repo && (
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

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search skills..."
            className="pl-8"
            value={q}
            onChange={(e) => updateParams({ q: e.target.value, page: "1" })}
          />
        </div>
        <Button
          variant="outline"
          className="gap-2 sm:w-auto"
          disabled
          title="Filtering options will be available in the next iteration"
        >
          <Filter className="h-4 w-4" /> Filter
        </Button>
        <Button
          variant="outline"
          className="gap-2 sm:w-auto"
          disabled
          title="Sorting options will be available in the next iteration"
        >
          <SortAsc className="h-4 w-4" /> Sort
        </Button>
      </div>

      <div className="flex items-center justify-between text-sm text-muted-foreground">
        <span>{loading ? "Loading skills..." : `Showing ${skills.length} skills on this page`}</span>
        <span>Page {page}</span>
      </div>

      <div className="space-y-3 md:hidden">
        {loading ? (
          Array.from({ length: 5 }).map((_, index) => (
            <div key={index} className="rounded-lg border bg-card p-4 space-y-3">
              <div className="h-4 w-32 animate-pulse rounded bg-muted" />
              <div className="h-3 w-full animate-pulse rounded bg-muted" />
              <div className="h-3 w-2/3 animate-pulse rounded bg-muted" />
              <div className="h-8 w-full animate-pulse rounded bg-muted" />
            </div>
          ))
        ) : skills.length === 0 ? (
          <div className="rounded-lg border bg-card p-6 text-center text-muted-foreground">
            No skills found.
          </div>
        ) : (
          skills.map((skill) => (
            <article key={skill.id} className="rounded-lg border bg-card p-4 space-y-3">
              <div className="flex items-start justify-between gap-3">
                <div className="space-y-1">
                  <Link
                    to={`/${skill.owner}/${skill.repo}/${skill.name}`}
                    className="font-semibold text-base hover:underline"
                  >
                    {skill.name}
                  </Link>
                  <p className="text-sm text-muted-foreground line-clamp-3">
                    {skill.description || "No description provided."}
                  </p>
                </div>
                {skill.latest_version && <Badge variant="secondary">{skill.latest_version}</Badge>}
              </div>

              <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
                <div>
                  by <Link to={`/${skill.owner}`} className="hover:underline">{skill.owner}</Link>
                  {" / "}
                  <Link to={`/${skill.owner}/${skill.repo}`} className="hover:underline">{skill.repo}</Link>
                </div>
                <span className="inline-flex items-center gap-1 whitespace-nowrap">
                  <CalendarDays className="h-3 w-3" />
                  {new Date(skill.created_at).toLocaleDateString()}
                </span>
              </div>

              <Button
                variant="outline"
                size="sm"
                className="w-full gap-2"
                onClick={() => onCopyInstallCommand(skill)}
                aria-label={`Copy install command for ${skill.name}`}
              >
                {copiedSkillId === skill.id ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                {copiedSkillId === skill.id ? "Copied install command" : "Copy install command"}
              </Button>
            </article>
          ))
        )}
      </div>

      <div className="hidden md:block border rounded-md">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[50px]">#</TableHead>
              <TableHead>Skill</TableHead>
              <TableHead className="w-[140px]">Latest Version</TableHead>
              <TableHead className="text-right">Created</TableHead>
              <TableHead className="w-[140px] text-right">Install</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {loading ? (
              <TableRow>
                <TableCell colSpan={5} className="text-center py-8">
                  Loading skills...
                </TableCell>
              </TableRow>
            ) : skills.length === 0 ? (
              <TableRow>
                <TableCell colSpan={5} className="text-center py-8">
                  No skills found.
                </TableCell>
              </TableRow>
            ) : (
              skills.map((skill, index) => (
                <TableRow key={skill.id} className="align-top">
                  <TableCell className="font-medium text-muted-foreground pt-5">
                    {(page - 1) * 20 + index + 1}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-col gap-1.5">
                      <Link
                        to={`/${skill.owner}/${skill.repo}/${skill.name}`}
                        className="font-semibold hover:underline text-base"
                      >
                        {skill.name}
                      </Link>
                      <span className="text-sm text-muted-foreground line-clamp-2">
                        {skill.description || "No description provided."}
                      </span>
                      <div className="text-xs text-muted-foreground">
                        by <Link to={`/${skill.owner}`} className="hover:underline">{skill.owner}</Link>
                        {" / "}
                        <Link to={`/${skill.owner}/${skill.repo}`} className="hover:underline">{skill.repo}</Link>
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
                  <TableCell className="text-right">
                    <Button
                      variant="outline"
                      size="sm"
                      className="gap-2"
                      onClick={() => onCopyInstallCommand(skill)}
                      aria-label={`Copy install command for ${skill.name}`}
                      title="Copy install command"
                    >
                      {copiedSkillId === skill.id ? (
                        <Check className="h-4 w-4" />
                      ) : (
                        <Copy className="h-4 w-4" />
                      )}
                      <span>{copiedSkillId === skill.id ? "Copied" : "Copy"}</span>
                    </Button>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
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
          disabled={skills.length < 20}
        >
          Next
        </Button>
      </div>
    </div>
  )
}
