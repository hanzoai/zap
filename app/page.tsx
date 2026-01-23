import Link from 'next/link';

export default function HomePage() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center bg-[var(--background)] text-[var(--foreground)]">
      <div className="container mx-auto px-4 py-16 text-center">
        {/* Hero */}
        <div className="mb-4 text-sm font-medium uppercase tracking-widest text-[var(--muted-foreground)]">
          Hanzo AI
        </div>
        <h1 className="mb-4 text-6xl font-bold tracking-tight md:text-7xl">
          <span className="bg-gradient-to-r from-green-400 to-emerald-500 bg-clip-text text-transparent">
            ZAP
          </span>
        </h1>
        <p className="mb-2 text-2xl font-medium text-[var(--foreground)]">
          Zero-copy Agent Protocol
        </p>
        <p className="mb-8 text-xl text-[var(--muted-foreground)]">
          One ZAP endpoint to rule all MCP servers.
        </p>

        {/* Stats */}
        <div className="mb-12 grid gap-6 md:grid-cols-3">
          <div className="rounded-lg border border-[var(--border)] bg-[var(--card)] p-6">
            <div className="mb-2 text-4xl font-bold text-green-500">40-50×</div>
            <div className="text-[var(--muted-foreground)]">Lower infra costs</div>
          </div>
          <div className="rounded-lg border border-[var(--border)] bg-[var(--card)] p-6">
            <div className="mb-2 text-4xl font-bold text-green-500">&lt;1μs</div>
            <div className="text-[var(--muted-foreground)]">Local latency</div>
          </div>
          <div className="rounded-lg border border-[var(--border)] bg-[var(--card)] p-6">
            <div className="mb-2 text-4xl font-bold text-green-500">0</div>
            <div className="text-[var(--muted-foreground)]">JSON parsing</div>
          </div>
        </div>

        {/* Code example */}
        <div className="mb-12 rounded-lg border border-[var(--border)] bg-[var(--card)] p-6 text-left font-mono text-sm">
          <div className="mb-2 text-[var(--muted-foreground)]"># Connect 12 MCP servers → 1 ZAP endpoint</div>
          <div className="text-green-400">$ zapd serve --port 9999</div>
          <div className="text-green-400">$ zapd add mcp --name github --url stdio://gh-mcp</div>
          <div className="text-green-400">$ zapd add mcp --name slack --url http://localhost:8080</div>
          <div className="text-green-400">$ zapd add mcp --name db --url zap+unix:///tmp/postgres.sock</div>
          <div className="mt-2 text-[var(--muted-foreground)]"># Agents connect once: zap://localhost:9999</div>
        </div>

        {/* CTA buttons */}
        <div className="flex flex-wrap justify-center gap-4">
          <Link
            href="/docs"
            className="rounded-lg bg-green-500 px-8 py-3 font-medium text-black transition hover:bg-green-400"
          >
            Get Started
          </Link>
          <Link
            href="/docs/whitepaper"
            className="rounded-lg border border-[var(--border)] px-8 py-3 font-medium text-[var(--foreground)] transition hover:border-[var(--muted-foreground)]"
          >
            Read Whitepaper
          </Link>
          <a
            href="https://github.com/hanzoai/mcp"
            className="rounded-lg border border-[var(--border)] px-8 py-3 font-medium text-[var(--foreground)] transition hover:border-[var(--muted-foreground)]"
          >
            GitHub
          </a>
        </div>

        {/* Tagline */}
        <div className="mt-16 text-sm text-[var(--muted-foreground)]">
          MCP made tool integration easy — but JSON everywhere doesn't scale.
          <br />
          ZAP is Cap'n Proto-native: low overhead, low memory, built for swarms.
          <br />
          <span className="mt-2 inline-block text-green-500">Welcome to Hanzo AI. All aboard for a greener agentic future.</span>
        </div>
      </div>
    </main>
  );
}
