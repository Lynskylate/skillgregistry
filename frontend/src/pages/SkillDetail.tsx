import { useEffect, useState } from "react"
import { useParams, Link } from "react-router-dom"
import axios from "axios"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { ChevronRight, Calendar, Star, Github, Terminal } from "lucide-react"

interface SkillDetail {
  skill: {
    id: number
    name: string
    latest_version: string
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
    readme_content: string
    created_at: string
  }[]
}

export default function SkillDetail() {
  const { owner, repo, name } = useParams()
  const [data, setData] = useState<SkillDetail | null>(null)
  const [loading, setLoading] = useState(true)
  const [activeTab, setActiveTab] = useState<"readme" | "versions">("readme")

  useEffect(() => {
    if (owner && repo && name) {
      fetchSkill()
    }
  }, [owner, repo, name])

  const fetchSkill = async () => {
    setLoading(true)
    try {
      const res = await axios.get(`/api/skills/${owner}/${repo}/${name}`)
      if (res.data.code === 200) {
        setData(res.data.data)
      }
    } catch (error) {
      console.error("Failed to fetch skill details", error)
    } finally {
      setLoading(false)
    }
  }

  if (loading) return <div className="p-8 text-center">Loading skill details...</div>
  if (!data) return <div className="p-8 text-center">Skill not found</div>

  const latestVersion = data.versions.find(v => v.version === data.skill.latest_version) || data.versions[0]

  return (
    <div className="container mx-auto py-8 space-y-8">
      {/* Breadcrumb */}
      <div className="flex items-center text-sm text-muted-foreground">
        <Link to="/" className="hover:text-primary">Skills</Link>
        <ChevronRight className="h-4 w-4 mx-2" />
        <Link to={`/${data.registry.owner}`} className="hover:text-primary">{data.registry.owner}</Link>
        <ChevronRight className="h-4 w-4 mx-2" />
        <Link to={`/${data.registry.owner}/${data.registry.name}`} className="hover:text-primary">{data.registry.name}</Link>
        <ChevronRight className="h-4 w-4 mx-2" />
        <span className="text-foreground font-medium">{data.skill.name}</span>
      </div>

      {/* Header */}
      <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
        <div>
          <h1 className="text-4xl font-bold tracking-tight mb-2">{data.skill.name}</h1>
          <div className="flex items-center gap-4 text-muted-foreground">
            <span className="flex items-center gap-1">
              <Github className="h-4 w-4" />
              {data.registry.owner}/{data.registry.name}
            </span>
            <span className="flex items-center gap-1">
              <Star className="h-4 w-4" />
              {data.registry.stars} stars
            </span>
            <Badge variant="secondary">v{data.skill.latest_version}</Badge>
          </div>
        </div>
        <div className="w-full md:w-auto">
           <div className="bg-zinc-950 text-zinc-50 px-4 py-3 rounded-md font-mono text-sm flex items-center gap-4 shadow-sm border border-zinc-800">
             <Terminal className="h-4 w-4 text-zinc-400" />
             <span>npx skills add {data.registry.owner}/{data.registry.name}/{data.skill.name}</span>
             <Button variant="ghost" size="sm" className="h-6 ml-auto text-zinc-400 hover:text-white">
               Copy
             </Button>
           </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
        {/* Main Content */}
        <div className="lg:col-span-3 space-y-6">
          <div className="flex items-center border-b">
            <Button 
              variant={activeTab === "readme" ? "default" : "ghost"}
              className="rounded-none border-b-2 border-transparent data-[state=active]:border-primary"
              onClick={() => setActiveTab("readme")}
            >
              README.md
            </Button>
            <Button 
              variant={activeTab === "versions" ? "default" : "ghost"}
              className="rounded-none border-b-2 border-transparent data-[state=active]:border-primary"
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
                        {new Date().toLocaleDateString()} {/* Mock date if not in version */}
                      </span>
                    </div>
                    <Button variant="outline" size="sm">
                      View
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Sidebar */}
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">About</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="text-sm text-muted-foreground">
                Published {new Date(data.skill.created_at).toLocaleDateString()}
              </div>
              <div className="text-sm text-muted-foreground">
                License: MIT (Mock)
              </div>
              <div className="pt-4 border-t">
                <h4 className="text-xs font-semibold uppercase text-muted-foreground mb-2">Maintainer</h4>
                <div className="flex items-center gap-2">
                  <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center font-bold text-primary">
                    {data.registry.owner[0].toUpperCase()}
                  </div>
                  <span className="text-sm font-medium">{data.registry.owner}</span>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}
