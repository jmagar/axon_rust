import { NextResponse } from 'next/server'

export interface ApiErrorBody {
  error: string
  code?: string
  errorId?: string
  detail?: string
}

/**
 * Build a consistent JSON error response used across all API routes.
 *
 * Shape: `{ error: string, code?: string, errorId?: string, detail?: string }`
 */
export function apiError(
  status: number,
  error: string,
  opts?: { code?: string; errorId?: string; detail?: string },
): NextResponse<ApiErrorBody> {
  const body: ApiErrorBody = { error }
  if (opts?.code) body.code = opts.code
  if (opts?.errorId) body.errorId = opts.errorId
  if (opts?.detail) body.detail = opts.detail
  return NextResponse.json(body, { status })
}

/**
 * Generate a unique error ID for unhandled errors (log correlation).
 */
export function makeErrorId(prefix: string): string {
  return globalThis.crypto?.randomUUID?.() ?? `${prefix}-${Date.now()}`
}
