'use client'

import { useEffect } from 'react'
import { useRouter } from 'next/navigation'

// Cognito redirects back here with ?code=... after hosted-UI sign-up. We do
// NOT exchange the code for tokens — sign-up alone is the goal for Phase 71.
// The `landing` Cognito client's confidential secret therefore stays server-
// side. A future phase that needs a signed-in landing experience will add a
// Route Handler (app/signup/callback/route.ts) that exchanges the code with
// the secret and sets a session cookie; this page would then redirect via
// the route handler instead of directly.
export default function SignupCallback() {
  const router = useRouter()

  useEffect(() => {
    const target = process.env.NEXT_PUBLIC_SIGNUP_REDIRECT_AFTER || '/'
    router.replace(target)
  }, [router])

  return (
    <main className="min-h-screen flex items-center justify-center bg-gray-50">
      <p className="text-gray-600">Welcome! Redirecting…</p>
    </main>
  )
}
