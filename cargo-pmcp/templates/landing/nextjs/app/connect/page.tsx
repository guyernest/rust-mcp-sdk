'use client'

import Header from '../components/Header'
import ConnectSnippet from '../components/ConnectSnippet'
import { useLandingConfig } from '@/lib/useLandingConfig'

export default function Connect() {
  const cfg = useLandingConfig()

  if (cfg === null) {
    return (
      <>
        <Header />
        <main className="mx-auto max-w-3xl px-6 py-12">
          <p className="text-gray-600">Loading connection instructions…</p>
        </main>
      </>
    )
  }
  if (cfg === 'error') {
    return (
      <>
        <Header />
        <main className="mx-auto max-w-3xl px-6 py-12">
          <p className="text-gray-600">
            Connection instructions unavailable. Please try again.
          </p>
        </main>
      </>
    )
  }

  return (
    <>
      <Header />
      <main className="mx-auto max-w-3xl px-6 py-12">
        <h1 className="text-3xl font-bold text-gray-900">
          Connect your MCP client
        </h1>
        <p className="mt-3 text-gray-600">
          Your account is created. Pick your MCP client below to finish
          connecting.
        </p>

        <ConnectSnippet
          title="Claude Desktop"
          instructions="Add this to your claude_desktop_config.json, then restart Claude Desktop."
          config={{ mcpServers: { server: { url: cfg.mcpEndpoint } } }}
        />

        <ConnectSnippet
          title="Claude Code"
          instructions="Add this to ~/.claude/claude.json, or run the equivalent `claude mcp add` command."
          command={`claude mcp add server ${cfg.mcpEndpoint}`}
        />

        <ConnectSnippet
          title="ChatGPT"
          instructions={`Open ChatGPT → Settings → Beta features → MCP Connectors → Add, and paste the URL: ${cfg.mcpEndpoint}`}
        />

        <ConnectSnippet
          title="Generic MCP CLI"
          instructions="Run this in your terminal with an MCP-capable CLI installed."
          command={`npx mcp-client ${cfg.mcpEndpoint}`}
        />
      </main>
    </>
  )
}
