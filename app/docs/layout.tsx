import Link from 'next/link';
import type { ReactNode } from 'react';

const navItems = [
  { href: '/docs', label: 'Getting Started' },
  { href: '/docs/whitepaper', label: 'Whitepaper' },
  { href: '/docs/architecture', label: 'Architecture' },
  { href: '/docs/tools', label: 'Tools Reference' },
  { href: '/docs/gateway', label: 'Gateway' },
  { href: '/docs/consensus', label: 'Consensus' },
  { href: '/docs/rust-api', label: 'Rust API' },
  { href: '/docs/transports', label: 'Transports' },
];

export default function DocsLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen">
      {/* Sidebar */}
      <aside className="w-64 border-r border-zinc-800 bg-zinc-950/50 p-6 hidden md:block">
        <div className="mb-8">
          <Link href="/" className="text-xl font-bold text-green-500">
            ZAP
          </Link>
          <p className="text-xs text-muted-foreground mt-1">v0.2.1 — Cap'n Proto RPC</p>
        </div>
        <nav className="space-y-1">
          {navItems.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className="block px-3 py-2 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-zinc-800/50 transition-colors"
            >
              {item.label}
            </Link>
          ))}
        </nav>
        <div className="mt-8 pt-8 border-t border-zinc-800">
          <a
            href="https://github.com/hanzoai/mcp"
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm text-muted-foreground hover:text-foreground"
          >
            GitHub →
          </a>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 p-8 md:p-12 max-w-4xl">
        {children}
      </main>
    </div>
  );
}
