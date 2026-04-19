'use client'

import { useState } from 'react'

type ConnectSnippetProps = {
  title: string
  instructions: string
  /** JSON config payload (for Claude Desktop / Claude Code style). */
  config?: unknown
  /** One-line shell command (for generic CLI usage). */
  command?: string
}

export default function ConnectSnippet({
  title,
  instructions,
  config,
  command,
}: ConnectSnippetProps) {
  const [copied, setCopied] = useState(false)

  const payload = config != null ? JSON.stringify(config, null, 2) : command

  const handleCopy = async () => {
    if (!payload) return
    try {
      await navigator.clipboard.writeText(payload)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      // Clipboard API may be blocked in insecure contexts (non-HTTPS). Fall
      // back to a prompt so the user can still copy manually.
      window.prompt('Copy:', payload)
    }
  }

  return (
    <section className="my-6 rounded-lg border border-gray-200 bg-white p-5 shadow-sm">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900">{title}</h2>
        {payload ? (
          <button
            type="button"
            onClick={handleCopy}
            className="rounded-md border border-gray-300 bg-gray-50 px-3 py-1 text-sm font-medium text-gray-700 transition hover:bg-gray-100"
          >
            {copied ? 'Copied!' : 'Copy'}
          </button>
        ) : null}
      </div>
      <p className="mt-2 text-sm text-gray-600">{instructions}</p>
      {payload ? (
        <pre className="mt-3 overflow-x-auto rounded-md bg-gray-900 p-4 text-sm text-gray-100">
          <code>{payload}</code>
        </pre>
      ) : null}
    </section>
  )
}
