import type { MDXComponents } from 'mdx/types';

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    ...components,
    h1: ({ children }) => (
      <h1 className="text-4xl font-bold mb-6 text-foreground">{children}</h1>
    ),
    h2: ({ children }) => (
      <h2 className="text-2xl font-semibold mt-8 mb-4 text-foreground">{children}</h2>
    ),
    h3: ({ children }) => (
      <h3 className="text-xl font-medium mt-6 mb-3 text-foreground">{children}</h3>
    ),
    p: ({ children }) => (
      <p className="mb-4 text-muted-foreground">{children}</p>
    ),
    pre: ({ children }) => (
      <pre className="bg-zinc-900 rounded-lg p-4 overflow-x-auto mb-4 text-sm">{children}</pre>
    ),
    code: ({ children }) => (
      <code className="bg-zinc-800 rounded px-1.5 py-0.5 text-green-400">{children}</code>
    ),
    table: ({ children }) => (
      <div className="overflow-x-auto mb-6">
        <table className="w-full border-collapse">{children}</table>
      </div>
    ),
    th: ({ children }) => (
      <th className="border border-zinc-700 px-4 py-2 bg-zinc-800 text-left font-semibold">{children}</th>
    ),
    td: ({ children }) => (
      <td className="border border-zinc-700 px-4 py-2">{children}</td>
    ),
    ul: ({ children }) => (
      <ul className="list-disc list-inside mb-4 space-y-2">{children}</ul>
    ),
    ol: ({ children }) => (
      <ol className="list-decimal list-inside mb-4 space-y-2">{children}</ol>
    ),
    li: ({ children }) => (
      <li className="text-muted-foreground">{children}</li>
    ),
    blockquote: ({ children }) => (
      <blockquote className="border-l-4 border-green-500 pl-4 italic my-4 text-muted-foreground">{children}</blockquote>
    ),
    a: ({ href, children }) => (
      <a href={href} className="text-green-400 hover:text-green-300 underline">{children}</a>
    ),
    strong: ({ children }) => (
      <strong className="font-semibold text-foreground">{children}</strong>
    ),
    hr: () => <hr className="border-zinc-700 my-8" />,
  };
}
