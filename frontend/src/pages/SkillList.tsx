import { useEffect, useState } from "react"
import { useParams, useSearchParams, Link } from "react-router-dom"
import axios from "axios"
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
import { Search, Filter, SortAsc, Download } from "lucide-react"

interface Skill {
  id: number
  name: string
  owner: string
  repo: string
  latest_version: string | null
  description: string | null
  created_at: string
}

interface ApiResponse<T> {
  code: number
  message: string
  data: T
  timestamp: number
}

export default function SkillList() {
  const { owner, repo } = useParams()
  const [searchParams, setSearchParams] = useSearchParams()
  const [skills, setSkills] = useState<Skill[]>([])
  const [loading, setLoading] = useState(false)
  const [total, setTotal] = useState(0) // Mocking total since API pagination is simple
  
  const q = searchParams.get("q") || ""
  const page = parseInt(searchParams.get("page") || "1")

  useEffect(() => {
    fetchSkills()
  }, [owner, repo, q, page])

  const fetchSkills = async () => {
    setLoading(true)
    try {
      const params: any = { page, per_page: 20 }
      if (q) params.q = q
      if (owner) params.owner = owner
      if (repo) params.repo = repo
      
      // Default sort for leaderboard style
      if (!owner && !repo && !q) {
        // Global list: sort by installs (mocked by created_at for now) or name
      }

      const res = await axios.get<ApiResponse<Skill[]>>("/api/skills", { params })
      if (res.data.code === 200) {
        setSkills(res.data.data)
      }
    } catch (error) {
      console.error("Failed to fetch skills", error)
    } finally {
      setLoading(false)
    }
  }

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault()
    // Trigger refetch by updating URL which triggers useEffect
  }

  return (
    <div className="container mx-auto py-8 space-y-6">
      {/* Header Section */}
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
           <div className="bg-zinc-900 text-zinc-50 p-4 rounded-lg flex items-center justify-between font-mono text-sm">
             <span>$ npx skills add &lt;owner/repo&gt;</span>
             <Button variant="ghost" size="sm" className="text-zinc-400 hover:text-white">
               Copy
             </Button>
           </div>
        )}
      </div>

      {/* Controls */}
      <div className="flex items-center gap-4">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search skills..."
            className="pl-8"
            value={q}
            onChange={(e) => setSearchParams(prev => {
              prev.set("q", e.target.value);
              prev.set("page", "1");
              return prev;
            })}
          />
        </div>
        <Button variant="outline" className="gap-2">
          <Filter className="h-4 w-4" /> Filter
        </Button>
        <Button variant="outline" className="gap-2">
          <SortAsc className="h-4 w-4" /> Sort
        </Button>
      </div>

      {/* Table */}
      <div className="border rounded-md">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[50px]">#</TableHead>
              <TableHead>Skill</TableHead>
              <TableHead>Latest Version</TableHead>
              <TableHead className="text-right">Created</TableHead>
              <TableHead className="w-[100px]"></TableHead>
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
                <TableRow key={skill.id}>
                  <TableCell className="font-medium text-muted-foreground">
                    {(page - 1) * 20 + index + 1}
                  </TableCell>
                  <TableCell>
                    <div className="flex flex-col">
                      <Link 
                        to={`/${skill.owner}/${skill.repo}/${skill.name}`}
                        className="font-semibold hover:underline text-base"
                      >
                        {skill.name}
                      </Link>
                      <span className="text-sm text-muted-foreground line-clamp-1">
                        {skill.description || "No description provided."}
                      </span>
                      <div className="text-xs text-muted-foreground mt-1">
                        by <Link to={`/${skill.owner}`} className="hover:underline">{skill.owner}</Link>
                        {" / "}
                        <Link to={`/${skill.owner}/${skill.repo}`} className="hover:underline">{skill.repo}</Link>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell>
                    {skill.latest_version && (
                      <Badge variant="secondary">{skill.latest_version}</Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-right text-muted-foreground">
                    {new Date(skill.created_at).toLocaleDateString()}
                  </TableCell>
                  <TableCell>
                    <Button variant="ghost" size="icon">
                      <Download className="h-4 w-4" />
                    </Button>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
      
      {/* Pagination */}
      <div className="flex items-center justify-end space-x-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => setSearchParams(prev => { prev.set("page", String(Math.max(1, page - 1))); return prev; })}
          disabled={page <= 1}
        >
          Previous
        </Button>
        <div className="text-sm font-medium">Page {page}</div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setSearchParams(prev => { prev.set("page", String(page + 1)); return prev; })}
          disabled={skills.length < 20}
        >
          Next
        </Button>
      </div>
    </div>
  )
}
