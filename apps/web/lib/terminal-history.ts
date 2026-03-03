const HISTORY_KEY = 'axon.web.terminal.history'
const LEGACY_HISTORY_KEY = 'axon.terminal.history'
const MAX_HISTORY = 500

/**
 * Persistent command history backed by localStorage.
 *
 * Instantiate once and keep the same instance across renders (via useRef).
 * All localStorage access is guarded so this is safe to construct during SSR —
 * the load/save calls simply no-op when window is unavailable.
 */
export class TerminalHistory {
  private entries: string[]
  private cursor: number

  constructor() {
    this.entries = this.load()
    this.cursor = this.entries.length
  }

  /**
   * Add a command to history.
   * Skips empty strings and deduplicates consecutive identical commands.
   * Trims the list to MAX_HISTORY and resets the cursor to the end.
   */
  push(cmd: string): void {
    const trimmed = cmd.trim()
    if (!trimmed) return
    if (this.entries.length > 0 && this.entries[this.entries.length - 1] === trimmed) {
      this.reset()
      return
    }
    this.entries.push(trimmed)
    if (this.entries.length > MAX_HISTORY) {
      this.entries = this.entries.slice(this.entries.length - MAX_HISTORY)
    }
    this.save()
    this.reset()
  }

  /**
   * Move cursor toward older entries (backward in time).
   * Returns the entry at the new cursor position, or undefined if already
   * at the oldest entry.
   */
  prev(): string | undefined {
    if (this.entries.length === 0) return undefined
    if (this.cursor <= 0) return this.entries[0]
    this.cursor--
    return this.entries[this.cursor]
  }

  /**
   * Move cursor toward newer entries (forward in time).
   * Returns the entry at the new cursor position, or undefined if the cursor
   * has moved past the most recent entry (i.e. back to a blank input line).
   */
  next(): string | undefined {
    if (this.cursor >= this.entries.length) return undefined
    this.cursor++
    if (this.cursor >= this.entries.length) return undefined
    return this.entries[this.cursor]
  }

  /**
   * Reset cursor to the end (past the last entry).
   * Call this after the user submits a command.
   */
  reset(): void {
    this.cursor = this.entries.length
  }

  /** Return all entries in chronological order (most recent last). */
  getAll(): readonly string[] {
    return this.entries
  }

  private load(): string[] {
    if (typeof window === 'undefined') return []
    try {
      const raw = localStorage.getItem(HISTORY_KEY) ?? localStorage.getItem(LEGACY_HISTORY_KEY)
      if (!raw) return []
      localStorage.setItem(HISTORY_KEY, raw)
      localStorage.removeItem(LEGACY_HISTORY_KEY)
      const parsed: unknown = JSON.parse(raw)
      if (!Array.isArray(parsed)) return []
      return parsed.filter((item): item is string => typeof item === 'string')
    } catch {
      return []
    }
  }

  private save(): void {
    if (typeof window === 'undefined') return
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(this.entries))
    } catch {
      // Ignore quota / private-browsing errors
    }
  }
}
