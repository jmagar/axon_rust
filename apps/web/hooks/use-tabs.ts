'use client'

import { useCallback, useEffect, useRef, useState } from 'react'

export interface EditorTab {
  id: string
  title: string
  markdown: string
  docFilename: string | null
}

const TABS_KEY = 'axon.web.editor.tabs'
const ACTIVE_KEY = 'axon.web.editor.active-tab'

function createTabId(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID()
  }
  return `tab-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
}

function newBlankTab(): EditorTab {
  return { id: createTabId(), title: 'Untitled', markdown: '', docFilename: null }
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

  // Refs that track current state values for use in callbacks without stale closures
  const tabsRef = useRef<EditorTab[]>(tabs)
  const activeTabIdRef = useRef(activeTabId)

  useEffect(() => {
    tabsRef.current = tabs
  }, [tabs])

  useEffect(() => {
    activeTabIdRef.current = activeTabId
  }, [activeTabId])

  useEffect(() => {
    const { tabs: t, activeTabId: a } = loadPersisted()
    setTabs(t)
    setActiveTabId(a)
    setHydrated(true)
  }, [])

  // Persist state after each change — gated on hydration to skip the empty initial state
  useEffect(() => {
    if (hydrated) persist(tabs, activeTabId)
  }, [tabs, activeTabId, hydrated])

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0] ?? null

  const openTab = useCallback((partial: Omit<EditorTab, 'id'>) => {
    const tab: EditorTab = { id: createTabId(), ...partial }
    setTabs((prev) => [...prev, tab])
    setActiveTabId(tab.id)
    return tab.id
  }, [])

  const closeTab = useCallback((id: string) => {
    const prev = tabsRef.current
    if (prev.length === 1) {
      // Never close the last tab — replace it with a blank one
      const blank = newBlankTab()
      setTabs([blank])
      setActiveTabId(blank.id)
      return
    }
    const idx = prev.findIndex((t) => t.id === id)
    const next = prev.filter((t) => t.id !== id)
    const newActive =
      id === activeTabIdRef.current
        ? (next[Math.max(0, idx - 1)]?.id ?? next[0]?.id ?? '')
        : activeTabIdRef.current
    setTabs(next)
    setActiveTabId(newActive)
  }, [])

  const activateTab = useCallback((id: string) => {
    setActiveTabId(id)
  }, [])

  const updateTab = useCallback((id: string, patch: Partial<Omit<EditorTab, 'id'>>) => {
    setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, ...patch } : t)))
  }, [])

  return { tabs, activeTabId, activeTab, hydrated, openTab, closeTab, activateTab, updateTab }
}
