import Header from '../components/Header'
import ConnectSnippet from '../components/ConnectSnippet'

// `MCP_SERVER_NAME` is injected by the pmcp.run deploy-landing Lambda (it's
// already the canonical identifier used in `pmcp proxy <name>` commands).
const serverName = process.env.MCP_SERVER_NAME || 'your-server'

const mcpClientConfig = {
  mcpServers: {
    [serverName]: {
      command: 'pmcp',
      args: ['proxy', serverName],
    },
  },
}

export default function Connect() {
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
          config={mcpClientConfig}
        />

        <ConnectSnippet
          title="Claude Code"
          instructions="Add this to ~/.claude/claude.json, or run `claude mcp add` with the same values."
          config={mcpClientConfig}
        />

        <ConnectSnippet
          title="ChatGPT"
          instructions={`Open ChatGPT → Settings → MCP Connectors → Add, and paste the URL: https://${serverName}.us-east.true-mcp.com`}
        />

        <ConnectSnippet
          title="Generic MCP CLI"
          instructions="Run this in your terminal with the pmcp CLI installed."
          command={`pmcp connect ${serverName}`}
        />
      </main>
    </>
  )
}
