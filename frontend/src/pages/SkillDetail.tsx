import { useEffect, useState } from "react"
import { useParams, Link } from "react-router-dom"
import { api } from "@/lib/api"
import type { ApiResponse } from "@/lib/types"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ChevronRight, Calendar, Star, Github, Terminal } from "lucide-react"

type SkillDetailPayload = {
  skill: {
    id: number
    name: string
    latest_version: string | null
    created_at: string
    updated_at: string
  }
  registry: {
    owner: string
    name: string
    stars: number
    url: string
  }
  versions: {
    version: string
    readme_content: string | null
    created_at: string
    metadata?: Record<string, unknown> | null
  }[]
  install_count: number
  last_synced_at: string | null
  license: string | null
  compatibility: string[] | null
  allowed_tools: string[] | null
  homepage: string | null
  documentation_url: string | null
}

type DownloadSkillResponse = {
  download_url: string
  expires_at: string
  md5: string | null
  version: string
  file_size: number | null
}

export default function SkillDetail() {
  const { host, org, repo, name } = useParams()
  const [data, setData] = useState<SkillDetailPayload | null>(null)
  const [loading, setLoading] = useState(true)
  const [activeTab, setActiveTab] = useState<"readme" | "versions">("readme")
  const [copiedInstallCommand, setCopiedInstallCommand] = useState(false)
  const [downloading, setDownloading] = useState(false)

  useEffect(() => {
    if (host && org && repo && name) {
      fetchSkill()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, org, repo, name])

  const fetchSkill = async () => {
    setLoading(true)
    try {
      const res = await api.get<ApiResponse<SkillDetailPayload>>(`/api/${host}/${org}/${repo}/skill/${name}`)
      if (res.data.code === 200 && res.data.data) {
        setData(res.data.data)
      } else {
        setData(null)
      }
    } catch (error) {
      console.error("Failed to fetch skill details", error)
      setData(null)
    } finally {
      setLoading(false)
    }
  }

  const copyText = async (text: string) => {
    // Try modern clipboard API first (only works in secure contexts)
    if (window.isSecureContext && navigator.clipboard) {
      try {
        await navigator.clipboard.writeText(text)
        return true
      } catch {
        // Fall through to fallback method
      }
    }

    // Fallback: use textarea method for non-secure contexts
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

  const onCopyInstallCommand = async () => {
    if (!data) return
    const copied = await copyText(`npx skills add ${data.registry.owner}/${data.registry.name} --skill ${data.skill.name}`)
    if (copied) {
      setCopiedInstallCommand(true)
      window.setTimeout(() => setCopiedInstallCommand(false), 1200)
    }
  }

  const downloadSkill = async () => {
    if (!host || !org || !repo || !name) return
    setDownloading(true)
    try {
      const res = await api.get<ApiResponse<DownloadSkillResponse>>(
        `/api/${host}/${org}/${repo}/skill/${name}/download`,
      )
      if (res.data.code === 200 && res.data.data) {
        window.location.href = res.data.data.download_url
      }
    } catch (error) {
      console.error("Failed to download skill", error)
    } finally {
      setDownloading(false)
    }
  }

  if (loading) {
    return (
      <div className="container mx-auto py-8">
        <div className="rounded-lg border bg-card p-10 text-center text-muted-foreground">
          Loading skill details...
        </div>
      </div>
    )
  }

  if (!data) {
    return (
      <div className="container mx-auto py-8">
        <div className="rounded-lg border bg-card p-10 text-center space-y-4">
          <p className="text-muted-foreground">Skill not found.</p>
          <Button asChild variant="outline">
            <Link to="/">Back to leaderboard</Link>
          </Button>
        </div>
      </div>
    )
  }

  const latestVersion = data.versions.find((v) => v.version === data.skill.latest_version) || data.versions[0]

  return (
    <div className="container mx-auto py-8 space-y-8">
      <div className="flex flex-wrap items-center text-sm text-muted-foreground">
        <Link to="/" className="hover:text-primary">Skills</Link>
        <ChevronRight className="h-4 w-4 mx-2" />
        <Link to={`/${host}`} className="hover:text-primary">{host}</Link>
        <ChevronRight className="h-4 w-4 mx-2" />
        <Link to={`/${host}/${org}`} className="hover:text-primary">{org}</Link>
        <ChevronRight className="h-4 w-4 mx-2" />
        <Link to={`/${host}/${org}/${repo}`} className="hover:text-primary">{repo}</Link>
        <ChevronRight className="h-4 w-4 mx-2" />
        <span className="text-foreground font-medium">{data.skill.name}</span>
      </div>

      <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
        <div>
          <h1 className="text-4xl font-bold tracking-tight mb-2">{data.skill.name}</h1>
          <div className="flex flex-wrap items-center gap-4 text-muted-foreground">
            <span className="flex items-center gap-1">
              <Github className="h-4 w-4" />
              {host}/{data.registry.owner}/{data.registry.name}
            </span>
            <span className="flex items-center gap-1">
              <Star className="h-4 w-4" />
              {data.registry.stars} stars
            </span>
            {data.skill.latest_version && <Badge variant="secondary">v{data.skill.latest_version}</Badge>}
          </div>
        </div>
        <div className="w-full md:w-auto space-y-2">
          <div className="bg-zinc-950 text-zinc-50 px-4 py-3 rounded-md font-mono text-sm flex items-center gap-4 shadow-sm border border-zinc-800">
            <Terminal className="h-4 w-4 text-zinc-400" />
            <span className="truncate">npx skills add {data.registry.owner}/{data.registry.name} --skill {data.skill.name}</span>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 ml-auto text-zinc-300 hover:text-white"
              onClick={onCopyInstallCommand}
              aria-label="Copy install command"
            >
              {copiedInstallCommand ? "Copied" : "Copy"}
            </Button>
          </div>
          <Button onClick={downloadSkill} disabled={downloading} className="w-full">
            {downloading ? "Preparing download..." : `Download ${data.skill.latest_version ?? "latest"}`}
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
        <div className="lg:col-span-3 space-y-6">
          <div className="flex items-center border-b">
            <Button
              variant="ghost"
              className={`rounded-none border-b-2 ${activeTab === "readme" ? "border-primary" : "border-transparent"}`}
              onClick={() => setActiveTab("readme")}
            >
              README.md
            </Button>
            <Button
              variant="ghost"
              className={`rounded-none border-b-2 ${activeTab === "versions" ? "border-primary" : "border-transparent"}`}
              onClick={() => setActiveTab("versions")}
            >
              Versions ({data.versions.length})
            </Button>
          </div>

          <div className="min-h-[400px]">
            {activeTab === "readme" && (
              <div className="prose dark:prose-invert max-w-none p-6 border rounded-lg bg-card">
                {latestVersion?.readme_content ? (
                  <Markdown remarkPlugins={[remarkGfm]}>{latestVersion.readme_content}</Markdown>
                ) : (
                  <div className="text-muted-foreground italic">No README available.</div>
                )}
              </div>
            )}

            {activeTab === "versions" && (
              <div className="border rounded-lg overflow-hidden">
                {data.versions.map((v) => (
                  <div key={v.version} className="flex items-center justify-between p-4 border-b last:border-0 hover:bg-muted/50">
                    <div className="flex items-center gap-4">
                      <span className="font-mono font-medium">{v.version}</span>
                      <span className="text-sm text-muted-foreground flex items-center gap-1">
                        <Calendar className="h-3 w-3" />
                        {new Date(v.created_at).toLocaleDateString()}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="space-y-6 lg:sticky lg:top-24 lg:self-start">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">About</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="text-sm text-muted-foreground">
                Published {new Date(data.skill.created_at).toLocaleDateString()}
              </div>
              <div className="text-sm text-muted-foreground">
                Installs: <span className="font-medium text-foreground">{data.install_count ?? 0}</span>
              </div>
              {data.last_synced_at && (
                <div className="text-sm text-muted-foreground">
                  Last synced: {new Date(data.last_synced_at).toLocaleString()}
                </div>
              )}
              {data.license && (
                <div>
                  <span className="text-sm text-muted-foreground">License: </span>
                  <Badge variant="secondary">{data.license}</Badge>
                </div>
              )}
              {data.compatibility && data.compatibility.length > 0 && (
                <div>
                  <span className="text-sm text-muted-foreground">Compatibility:</span>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {data.compatibility.map((tag, index) => (
                      <Badge key={`${tag}-${index}`} variant="outline">{tag}</Badge>
                    ))}
                  </div>
                </div>
              )}
              {data.allowed_tools && data.allowed_tools.length > 0 && (
                <div>
                  <span className="text-sm text-muted-foreground">Allowed Tools:</span>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {data.allowed_tools.map((tool, index) => (
                      <Badge key={`${tool}-${index}`} variant="outline">{tool}</Badge>
                    ))}
                  </div>
                </div>
              )}
              {data.homepage && (
                <a href={data.homepage} target="_blank" rel="noopener noreferrer" className="text-sm text-primary hover:underline block">
                  Homepage
                </a>
              )}
              {data.documentation_url && (
                <a
                  href={data.documentation_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-primary hover:underline block"
                >
                  Documentation
                </a>
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}
