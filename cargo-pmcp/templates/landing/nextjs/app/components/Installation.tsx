'use client'

import { useState } from 'react'

// Environment variables are baked at build time
const serverName = process.env.MCP_SERVER_NAME || 'server'
const endpoint = process.env.MCP_ENDPOINT || 'https://your-endpoint.com'

// Generate Cursor install link (base64 encoded config)
// Format: cursor://anysphere.cursor-deeplink/mcp/install?name=NAME&config=BASE64_CONFIG
function getCursorInstallLink(): string {
  const config = JSON.stringify({ url: endpoint })
  // Use btoa for base64 encoding (works in browser)
  const encodedConfig = typeof window !== 'undefined' ? btoa(config) : Buffer.from(config).toString('base64')
  return `cursor://anysphere.cursor-deeplink/mcp/install?name=${encodeURIComponent(serverName)}&config=${encodedConfig}`
}

interface ClientConfig {
  id: string
  name: string
  config: string
  instructions: string[]
  installLink?: string
  installLinkText?: string
}

const clients: ClientConfig[] = [
  {
    id: 'claude',
    name: 'Claude Desktop',
    config: `{
  "mcpServers": {
    "${serverName}": {
      "url": "${endpoint}"
    }
  }
}`,
    instructions: [
      'Open Claude Desktop settings',
      'Navigate to the "Developer" tab',
      'Add this to your claude_desktop_config.json:',
    ],
  },
  {
    id: 'cursor',
    name: 'Cursor',
    config: `{
  "mcpServers": {
    "${serverName}": {
      "url": "${endpoint}"
    }
  }
}`,
    instructions: [
      'Click the button below for one-click installation, or manually add to ~/.cursor/mcp.json:',
    ],
    installLink: getCursorInstallLink(),
    installLinkText: 'Install in Cursor',
  },
  {
    id: 'chatgpt',
    name: 'ChatGPT',
    config: `Server Name: ${serverName}
Server URL: ${endpoint}`,
    instructions: [
      'Open ChatGPT settings',
      'Go to "Integrations" or "MCP Servers"',
      'Add a new MCP server with:',
    ],
  },
  {
    id: 'cline',
    name: 'Cline (VS Code)',
    config: `{
  "cline.mcpServers": {
    "${serverName}": {
      "url": "${endpoint}"
    }
  }
}`,
    instructions: [
      'Open VS Code settings (JSON)',
      'Add this configuration:',
    ],
  },
]

export default function Installation() {
  const [selectedClient, setSelectedClient] = useState('claude')

  const client = clients.find((c) => c.id === selectedClient) || clients[0]

  return (
    <div id="installation" className="container mx-auto px-4 py-20 bg-gray-50">
      <h2 className="text-3xl font-bold text-center mb-12">Installation</h2>

      <div className="max-w-4xl mx-auto">
        {/* Client selector */}
        <div className="flex flex-wrap gap-4 mb-8 justify-center">
          {clients.map((c) => (
            <button
              key={c.id}
              onClick={() => setSelectedClient(c.id)}
              className={`px-6 py-3 rounded-lg font-semibold transition-colors ${
                selectedClient === c.id
                  ? 'bg-blue-600 text-white'
                  : 'bg-white text-gray-700 hover:bg-gray-100 border border-gray-200'
              }`}
            >
              {c.name}
            </button>
          ))}
        </div>

        {/* Instructions */}
        <div className="bg-white p-8 rounded-xl shadow-lg">
          <h3 className="text-2xl font-semibold mb-6">{client.name}</h3>

          <ol className="space-y-4 mb-6">
            {client.instructions.map((step, i) => (
              <li key={i} className="flex items-start">
                <span className="flex-shrink-0 w-8 h-8 bg-blue-600 text-white rounded-full flex items-center justify-center mr-4 text-sm font-bold">
                  {i + 1}
                </span>
                <span className="pt-1 text-gray-700">{step}</span>
              </li>
            ))}
          </ol>

          {/* One-click install button (if available) */}
          {client.installLink && (
            <div className="mb-6">
              <a
                href={client.installLink}
                className="inline-flex items-center px-6 py-3 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded-lg transition-colors"
              >
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                </svg>
                {client.installLinkText || 'Install'}
              </a>
              <p className="mt-2 text-sm text-gray-500">
                Or copy the configuration below for manual setup:
              </p>
            </div>
          )}

          <div className="relative">
            <pre className="bg-gray-900 text-gray-100 p-4 rounded-lg overflow-x-auto">
              <code>{client.config}</code>
            </pre>
            <button
              onClick={() => navigator.clipboard.writeText(client.config)}
              className="absolute top-2 right-2 px-3 py-1 bg-gray-700 hover:bg-gray-600 text-white text-sm rounded transition-colors"
            >
              Copy
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
