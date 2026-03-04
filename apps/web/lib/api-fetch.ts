/**
 * Drop-in replacement for fetch() for all /api/* calls.
 * Automatically injects x-api-key when NEXT_PUBLIC_AXON_API_TOKEN is set.
 */

const API_TOKEN = process.env.NEXT_PUBLIC_AXON_API_TOKEN

export function apiFetch(input: string | URL | Request, init?: RequestInit): Promise<Response> {
  if (!API_TOKEN) return fetch(input, init)
  const headers = new Headers(init?.headers)
  if (!headers.has('x-api-key')) headers.set('x-api-key', API_TOKEN)
  return fetch(input, { ...init, headers })
}
