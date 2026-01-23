import './global.css';
import type { ReactNode } from 'react';

export const metadata = {
  title: 'ZAP - Zero-copy Agent Protocol',
  description: 'One ZAP endpoint to rule all MCP servers. The MCP Killer.',
};

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className="dark">
      <body className="min-h-screen bg-background text-foreground antialiased">
        {children}
      </body>
    </html>
  );
}
