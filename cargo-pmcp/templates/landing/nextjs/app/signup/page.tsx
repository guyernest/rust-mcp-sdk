'use client'

import { useEffect } from 'react'

// Client-side redirect to the server's Cognito Managed Login hosted UI in
// sign-up mode. The four NEXT_PUBLIC_* values are injected at build time by
// the pmcp.run deploy-landing Lambda (via Amplify app-level environmentVariables)
// — see rust-mcp-sdk/cargo-pmcp/CHANGES.md for the contract.
export default function Signup() {
  useEffect(() => {
    const domain = process.env.NEXT_PUBLIC_COGNITO_DOMAIN
    const region = process.env.NEXT_PUBLIC_COGNITO_REGION
    const clientId = process.env.NEXT_PUBLIC_LANDING_CLIENT_ID

    if (!domain || !region || !clientId) {
      console.error(
        'Sign-up is not configured: missing NEXT_PUBLIC_COGNITO_DOMAIN, ' +
          'NEXT_PUBLIC_COGNITO_REGION, or NEXT_PUBLIC_LANDING_CLIENT_ID. ' +
          'These are injected by the pmcp.run platform at deploy time.',
      )
      return
    }

    const origin = window.location.origin
    const url = new URL(
      `https://${domain}.auth.${region}.amazoncognito.com/signup`,
    )
    url.searchParams.set('client_id', clientId)
    url.searchParams.set('response_type', 'code')
    url.searchParams.set('scope', 'openid email')
    url.searchParams.set('redirect_uri', `${origin}/signup/callback`)

    window.location.replace(url.toString())
  }, [])

  return (
    <main className="min-h-screen flex items-center justify-center bg-gray-50">
      <p className="text-gray-600">Redirecting to sign-up…</p>
    </main>
  )
}
