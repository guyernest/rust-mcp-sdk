'use client'
import { useEffect, useState } from 'react'

export type LandingConfig = {
  /** Public MCP endpoint — always present. */
  mcpEndpoint: string
  /** Sign-up config, or null when OAuth is not enabled / landing client missing. */
  signup: {
    cognitoDomain: string
    cognitoRegion: string
    landingClientId: string
    signupRedirectAfter: string
  } | null
}

/**
 * Resolved states:
 *   null            — still loading
 *   'error'         — non-OK HTTP response, network failure, or non-JSON parse failure
 *   LandingConfig   — successful load (signup may itself be null when disabled)
 *
 * Note: the hook does NOT validate the JSON shape of a 200 response. Any 200 with a
 * JSON body is accepted as-is and cast to LandingConfig. This matches the platform
 * contract (CR-03 §Contract): the /landing-config endpoint guarantees shape or serves
 * a non-200 status. Consumers must treat loaded `cfg` as trusted input from the
 * platform.
 */
export type LandingConfigState = LandingConfig | null | 'error'

export function useLandingConfig(): LandingConfigState {
  const [cfg, setCfg] = useState<LandingConfigState>(null)
  useEffect(() => {
    // Cancel flag: prevents setCfg after unmount (ignore-after-unmount — the fetch
    // itself is NOT aborted; there is no abort-controller by design per CR-03 §1).
    let cancelled = false
    fetch('/landing-config', { cache: 'no-store' })
      .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
      .then((c) => { if (!cancelled) setCfg(c as LandingConfig) })
      .catch(() => { if (!cancelled) setCfg('error') })
    return () => { cancelled = true }
  }, [])
  return cfg
}
