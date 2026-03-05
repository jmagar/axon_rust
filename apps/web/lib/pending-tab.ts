const KEY = 'axon.web.editor.pending-tab'

export interface PendingTab {
  id: string
  title: string
  markdown: string
}

export function setPendingTab(tab: Omit<PendingTab, 'id'>): void {
  const entry: PendingTab = { id: crypto.randomUUID(), ...tab }
  localStorage.setItem(KEY, JSON.stringify(entry))
  // Notify same-page listeners (cross-page gets native storage event)
  window.dispatchEvent(
    new StorageEvent('storage', {
      key: KEY,
      newValue: JSON.stringify(entry),
      storageArea: localStorage,
    }),
  )
}

export function consumePendingTab(): PendingTab | null {
  const raw = localStorage.getItem(KEY)
  if (!raw) return null
  localStorage.removeItem(KEY)
  try {
    return JSON.parse(raw) as PendingTab
  } catch {
    return null
  }
}

export function onPendingTab(cb: (tab: PendingTab) => void): () => void {
  const handler = (e: StorageEvent) => {
    if (e.key !== KEY || !e.newValue) return
    try {
      cb(JSON.parse(e.newValue) as PendingTab)
    } catch {
      // ignore malformed
    }
  }
  window.addEventListener('storage', handler)
  return () => window.removeEventListener('storage', handler)
}
