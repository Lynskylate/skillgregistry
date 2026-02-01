import { BrowserRouter as Router, Routes, Route, Link, useParams } from 'react-router-dom';
import { useState, useEffect } from 'react';
import axios from 'axios';

// Interfaces
interface Skill {
  name: string;
  latest_version: string;
  description: string;
  created_at: string;
}

interface SkillDetail {
  skill: {
    id: number;
    name: string;
    latest_version: string;
    created_at: string;
  };
  versions: Array<{
    version: string;
    description: string;
    readme_content: string;
    created_at: string;
  }>;
}

function HomePage() {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchSkills();
  }, []);

  const fetchSkills = async (q?: string) => {
    setLoading(true);
    try {
      const url = q ? `http://localhost:3000/api/skills?q=${q}` : `http://localhost:3000/api/skills`;
      const res = await axios.get(url);
      setSkills(res.data);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    fetchSkills(search);
  };

  return (
    <div className="container mx-auto p-4 max-w-6xl">
      <div className="flex flex-col items-center mb-12 mt-8">
        <h1 className="text-4xl font-bold mb-4 tracking-tight">Agent Skills Registry</h1>
        <p className="text-muted-foreground mb-8">Discover and share capabilities for AI agents</p>
        <form onSubmit={handleSearch} className="w-full max-w-lg flex gap-2">
          <input 
            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            placeholder="Search skills..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <button type="submit" className="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 bg-black text-white hover:bg-gray-800 h-10 px-4 py-2">
            Search
          </button>
        </form>
      </div>

      {loading ? (
        <div className="text-center">Loading skills...</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {skills.map((skill) => (
            <div key={skill.name} className="rounded-lg border bg-card text-card-foreground shadow-sm hover:shadow-md transition-shadow">
              <div className="p-6 flex flex-col items-start gap-4 h-full">
                <div className="flex justify-between w-full items-start">
                  <h3 className="text-xl font-semibold leading-none tracking-tight">
                    <Link to={`/skill/${skill.name}`} className="hover:underline text-blue-600">{skill.name}</Link>
                  </h3>
                  {skill.latest_version && (
                    <span className="inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 border-transparent bg-gray-100 text-gray-800">
                      v{skill.latest_version}
                    </span>
                  )}
                </div>
                <p className="text-sm text-gray-600 flex-grow">{skill.description || "No description available."}</p>
                <div className="text-xs text-gray-400 mt-2">
                  Updated {new Date(skill.created_at).toLocaleDateString()}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
      {!loading && skills.length === 0 && (
        <div className="text-center text-gray-500 mt-12">No skills found. Try a different search term.</div>
      )}
    </div>
  );
}

function SkillPage() {
  const { name } = useParams<{ name: string }>();
  const [data, setData] = useState<SkillDetail | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (name) {
      axios.get(`http://localhost:3000/api/skills/${name}`)
        .then(res => setData(res.data))
        .catch(err => console.error(err))
        .finally(() => setLoading(false));
    }
  }, [name]);

  if (loading) return <div className="container mx-auto p-4">Loading...</div>;
  if (!data) return <div className="container mx-auto p-4">Skill not found</div>;

  const latest = data.versions.find(v => v.version === data.skill.latest_version) || data.versions[0];

  return (
    <div className="container mx-auto p-4 max-w-4xl">
      <div className="mb-6">
        <Link to="/" className="text-blue-600 hover:underline">&larr; Back to Search</Link>
      </div>
      
      <div className="border rounded-lg p-8 shadow-sm bg-white">
        <div className="flex justify-between items-start mb-6">
          <div>
            <h1 className="text-3xl font-bold mb-2">{data.skill.name}</h1>
            <p className="text-gray-600">{latest?.description}</p>
          </div>
          <div className="flex flex-col items-end gap-2">
            <span className="text-lg font-semibold">Latest: v{data.skill.latest_version}</span>
            <span className="text-sm text-gray-500">Released {new Date(latest?.created_at).toLocaleDateString()}</span>
          </div>
        </div>

        <div className="border-t pt-6">
          <h2 className="text-xl font-semibold mb-4">README</h2>
          <div className="prose max-w-none bg-gray-50 p-4 rounded-md">
            {/* Simple pre-wrap for now, ideally use a Markdown renderer */}
            <pre className="whitespace-pre-wrap font-sans text-sm">{latest?.readme_content}</pre>
          </div>
        </div>

        <div className="mt-8 border-t pt-6">
          <h2 className="text-xl font-semibold mb-4">Versions</h2>
          <div className="space-y-2">
            {data.versions.map(v => (
              <div key={v.version} className="flex justify-between items-center p-3 hover:bg-gray-50 rounded border">
                <span className="font-medium">v{v.version}</span>
                <span className="text-sm text-gray-500">{new Date(v.created_at).toLocaleDateString()}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/skill/:name" element={<SkillPage />} />
      </Routes>
    </Router>
  )
}

export default App
