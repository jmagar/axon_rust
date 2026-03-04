/**
 * Drop-in replacement for fetch() for all /api/* calls.
 * Automatically injects x-api-key when NEXT_PUBLIC_AXON_API_TOKEN is set.
 */

const API_TOKEN = process.env.NEXT_PUBLIC_AXON_API_TOKEN

function shouldInjectToken(input: string | URL | Request): boolean {
  try {
    const url =
      input instanceof Request ? new URL(input.url) : new URL(input, globalThis.location?.origin)
    if (url.pathname.startsWith('/api/')) return true
    if (globalThis.location && url.origin === globalThis.location.origin) return true
    return false
  } catch {
    // Relative URLs (e.g. "/api/foo") always start with /api/ and are same-origin
    if (typeof input === 'string' && input.startsWith('/api/')) return true
    return false
  }
}

export function apiFetch(input: string | URL | Request, init?: RequestInit): Promise<Response> {
  if (!API_TOKEN) return fetch(input, init)

  // Merge headers from Request input (if any) with init headers, then inject token
  const base = input instanceof Request ? input.headers : undefined
  const headers = new Headers(base)
  // init headers override Request headers
  if (init?.headers) {
    const override = new Headers(init.headers)
    override.forEach((value, key) => headers.set(key, value))
  }

  if (shouldInjectToken(input) && !headers.has('x-api-key')) {
    headers.set('x-api-key', API_TOKEN)
  }

  return fetch(input, { ...init, headers })
}
