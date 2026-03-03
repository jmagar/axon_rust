/**
 * Typed localStorage helpers. Wraps get/set/remove in try-catch
 * to gracefully handle private browsing, quota errors, and SSR.
 */

export function getStorageItem(key: string): string | null {
  if (typeof window === 'undefined') return null
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

export function setStorageItem(key: string, value: string): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(key, value)
  } catch {
    // Ignore quota or access errors.
  }
}

export function removeStorageItem(key: string): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.removeItem(key)
  } catch {
    // Ignore access errors.
  }
}

export function getStorageJson<T>(key: string): T | null {
  const raw = getStorageItem(key)
  if (!raw) return null
  try {
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

export function setStorageJson(key: string, value: unknown): void {
  if (typeof window === 'undefined') return
  try {
    setStorageItem(key, JSON.stringify(value))
  } catch {
    // Ignore serialization or quota errors.
  }
}
