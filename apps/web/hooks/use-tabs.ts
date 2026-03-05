'use client'

import { useCallback, useEffect, useState } from 'react'

export interface EditorTab {
  id: string
  title: string
  markdown: string
  docFilename: string | null
}

const TABS_KEY = 'axon.web.editor.tabs'
const ACTIVE_KEY = 'axon.web.editor.active-tab'

function newBlankTab(): EditorTab {
  return { id: crypto.randomUUID(), title: 'Untitled', markdown: '', docFilename: null }
}

function loadPersisted(): { tabs: EditorTab[]; activeTabId: string } {
  try {
    const rawTabs = localStorage.getItem(TABS_KEY)
    const rawActive = localStorage.getItem(ACTIVE_KEY)
    const tabs = rawTabs ? (JSON.parse(rawTabs) as EditorTab[]) : []
    if (tabs.length === 0) {
      const blank = newBlankTab()
      return { tabs: [blank], activeTabId: blank.id }
    }
    const activeTabId =
      rawActive && tabs.some((t) => t.id === rawActive) ? rawActive : (tabs[0]?.id ?? '')
    return { tabs, activeTabId }
  } catch {
    const blank = newBlankTab()
    return { tabs: [blank], activeTabId: blank.id }
  }
}

function persist(tabs: EditorTab[], activeTabId: string): void {
  try {
    localStorage.setItem(TABS_KEY, JSON.stringify(tabs))
    localStorage.setItem(ACTIVE_KEY, activeTabId)
  } catch {
    // ignore storage quota errors
  }
}

export function useTabs() {
  const [tabs, setTabs] = useState<EditorTab[]>([])
  const [activeTabId, setActiveTabId] = useState('')
  const [hydrated, setHydrated] = useState(false)

  useEffect(() => {
    const { tabs: t, activeTabId: a } = loadPersisted()
    setTabs(t)
    setActiveTabId(a)
    setHydrated(true)
  }, [])

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0] ?? null

  const openTab = useCallback((partial: Omit<EditorTab, 'id'>) => {
    const tab: EditorTab = { id: crypto.randomUUID(), ...partial }
    setTabs((prev) => {
      const next = [...prev, tab]
      persist(next, tab.id)
      return next
    })
    setActiveTabId(tab.id)
    return tab.id
  }, [])

  const closeTab = useCallback(
    (id: string) => {
      setTabs((prev) => {
        if (prev.length === 1) {
          // Never close the last tab — replace it with a blank one
          const blank = newBlankTab()
          persist([blank], blank.id)
          setActiveTabId(blank.id)
          return [blank]
        }
        const idx = prev.findIndex((t) => t.id === id)
        const next = prev.filter((t) => t.id !== id)
        const newActive =
          id === activeTabId ? (next[Math.max(0, idx - 1)]?.id ?? next[0]?.id ?? '') : activeTabId
        persist(next, newActive)
        setActiveTabId(newActive)
        return next
      })
    },
    [activeTabId],
  )

  const activateTab = useCallback((id: string) => {
    setActiveTabId(id)
    setTabs((prev) => {
      persist(prev, id)
      return prev
    })
  }, [])

  const updateTab = useCallback(
    (id: string, patch: Partial<Omit<EditorTab, 'id'>>) => {
      setTabs((prev) => {
        const next = prev.map((t) => (t.id === id ? { ...t, ...patch } : t))
        persist(next, activeTabId)
        return next
      })
    },
    [activeTabId],
  )

  return { tabs, activeTabId, activeTab, hydrated, openTab, closeTab, activateTab, updateTab }
}
