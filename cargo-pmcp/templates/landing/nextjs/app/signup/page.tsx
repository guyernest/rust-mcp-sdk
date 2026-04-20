'use client'

import { useEffect } from 'react'
import { useLandingConfig } from '@/lib/useLandingConfig'

// Client-side redirect to the server's Cognito Managed Login hosted UI in
// sign-up mode. Config comes from a runtime fetch of /landing-config served
// same-origin by the pmcp.run platform (Amplify 200-rewrite -> API GW -> Lambda).
// See rust-mcp-sdk/cargo-pmcp/CHANGES.md and CR-03 rev-2 for the contract.
export default function Signup() {
  const cfg = useLandingConfig()

  useEffect(() => {
    if (cfg === null || cfg === 'error') return
    if (!cfg.signup) return // rendered as "not available" below

    const { cognitoDomain, cognitoRegion, landingClientId } = cfg.signup
    const origin = window.location.origin
    const url = new URL(
      `https://${cognitoDomain}.auth.${cognitoRegion}.amazoncognito.com/signup`,
    )
    url.searchParams.set('client_id', landingClientId)
    url.searchParams.set('response_type', 'code')
    url.searchParams.set('scope', 'openid email')
    url.searchParams.set('redirect_uri', `${origin}/signup/callback`)

    window.location.replace(url.toString())
  }, [cfg])

  const disabled = cfg === 'error' || (cfg !== null && !cfg.signup)
  return (
    <main className="min-h-screen flex items-center justify-center bg-gray-50">
      <p className="text-gray-600">
        {disabled ? 'Sign-up is not available for this server.' : 'Redirecting to sign-up…'}
      </p>
    </main>
  )
}
