import Link from 'next/link'

const serverName = process.env.MCP_SERVER_NAME || 'MCP Server'

export default function Header() {
  return (
    <header className="border-b border-gray-200 bg-white">
      <nav className="mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
        <Link href="/" className="text-lg font-semibold text-gray-900">
          {serverName}
        </Link>
        <div className="flex items-center gap-4">
          <Link
            href="/connect"
            className="text-sm font-medium text-gray-700 hover:text-gray-900"
          >
            Connect
          </Link>
          <Link
            href="/signup"
            className="rounded-md bg-blue-600 px-4 py-2 text-sm font-semibold text-white shadow-sm transition hover:bg-blue-700"
          >
            Sign up
          </Link>
        </div>
      </nav>
    </header>
  )
}
