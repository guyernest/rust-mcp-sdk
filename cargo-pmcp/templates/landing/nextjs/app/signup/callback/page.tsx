'use client'

import { useEffect } from 'react'
import { useRouter } from 'next/navigation'
import { useLandingConfig } from '@/lib/useLandingConfig'

// Cognito redirects back here with ?code=... after hosted-UI sign-up. We do
// NOT exchange the code for tokens — sign-up alone is the goal for Phase 71.
// The `landing` Cognito client's confidential secret therefore stays server-
// side. The post-signup target comes from /landing-config at runtime
// (cfg.signup.signupRedirectAfter); fallback is '/connect'. A future phase
// that needs a signed-in landing experience will add a Route Handler
// (app/signup/callback/route.ts) that exchanges the code with the secret and
// sets a session cookie; this page would then redirect via the route handler
// instead of directly.
export default function SignupCallback() {
  const router = useRouter()
  const cfg = useLandingConfig()

  useEffect(() => {
    if (cfg === null) return // still loading
    const redirect =
      (cfg !== 'error' && cfg.signup?.signupRedirectAfter) || '/connect'
    router.replace(redirect)
  }, [cfg, router])

  return (
    <main className="min-h-screen flex items-center justify-center bg-gray-50">
      <p className="text-gray-600">Welcome! Redirecting…</p>
    </main>
  )
}
