'use client'

import { useCallback, useEffect, useRef, useState } from 'react'

const DESKTOP_SPLIT_STORAGE_KEY = 'axon.web.pulse.editor-split.desktop'
const MOBILE_SPLIT_STORAGE_KEY = 'axon.web.pulse.editor-split.mobile'
const SHOW_CHAT_STORAGE_KEY = 'axon.web.pulse.show-chat'
const SHOW_EDITOR_STORAGE_KEY = 'axon.web.pulse.show-editor'
export const MOBILE_PANE_STORAGE_KEY = 'axon.web.pulse.mobile-pane'

export function useSplitPane() {
  const [desktopSplitPercent, setDesktopSplitPercent] = useState(50)
  const [mobileSplitPercent, setMobileSplitPercent] = useState(56)
  const [isDesktop, setIsDesktop] = useState(false)
  const [mobilePane, setMobilePane] = useState<'chat' | 'editor'>('chat')
  const [showChat, setShowChat] = useState(true)
  const [showEditor, setShowEditor] = useState(true)

  const desktopSplitPercentRef = useRef(50)
  const mobileSplitPercentRef = useRef(56)
  const dragStartRef = useRef<{ pointerX: number; startPercent: number } | null>(null)
  const splitContainerRef = useRef<HTMLDivElement>(null)
  const splitHandleRef = useRef<HTMLDivElement>(null)
  const showEditorRef = useRef(true)
  const showChatRef = useRef(true)

  // Keep refs in sync with state
  useEffect(() => {
    desktopSplitPercentRef.current = desktopSplitPercent
  }, [desktopSplitPercent])
  useEffect(() => {
    mobileSplitPercentRef.current = mobileSplitPercent
  }, [mobileSplitPercent])
  useEffect(() => {
    showEditorRef.current = showEditor
  }, [showEditor])
  useEffect(() => {
    showChatRef.current = showChat
  }, [showChat])

  // Storage restore effect
  useEffect(() => {
    try {
      const desktop = window.localStorage.getItem(DESKTOP_SPLIT_STORAGE_KEY)
      const mobile = window.localStorage.getItem(MOBILE_SPLIT_STORAGE_KEY)
      const parsedDesktop = Number(desktop)
      const parsedMobile = Number(mobile)
      if (Number.isFinite(parsedDesktop) && parsedDesktop >= 20 && parsedDesktop <= 80) {
        setDesktopSplitPercent(parsedDesktop)
      }
      if (Number.isFinite(parsedMobile) && parsedMobile >= 35 && parsedMobile <= 70) {
        setMobileSplitPercent(parsedMobile)
      }
      const pane = window.localStorage.getItem(MOBILE_PANE_STORAGE_KEY)
      if (pane === 'chat' || pane === 'editor') setMobilePane(pane)
    } catch {
      // Ignore storage errors.
    }
  }, [])

  // Media query effect
  useEffect(() => {
    const media = window.matchMedia('(min-width: 1024px)')
    const update = () => setIsDesktop(media.matches)
    update()
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [])

  // Horizontal drag effect — click (< 4px) toggles editor; drag (>= 4px) resizes
  useEffect(() => {
    function onPointerMove(event: PointerEvent) {
      const start = dragStartRef.current
      const container = splitContainerRef.current
      if (!start || !container) return
      const rect = container.getBoundingClientRect()
      if (rect.width <= 0) return
      const deltaPx = event.clientX - start.pointerX
      const deltaPercent = (deltaPx / rect.width) * 100
      const next = Math.max(20, Math.min(80, start.startPercent + deltaPercent))
      setDesktopSplitPercent(next)
    }

    function stopDrag(event: PointerEvent) {
      const start = dragStartRef.current
      if (!start) return
      const totalMovement = Math.abs(event.clientX - start.pointerX)
      dragStartRef.current = null
      splitHandleRef.current?.classList.remove('bg-[rgba(175,215,255,0.15)]')
      if (totalMovement < 4) {
        // Click — toggle the editor panel; block collapse if chat is already collapsed
        const next = !showEditorRef.current
        if (!next && !showChatRef.current) return
        setShowEditor(next)
        try {
          window.localStorage.setItem(SHOW_EDITOR_STORAGE_KEY, String(next))
        } catch {
          /* ignore */
        }
        return
      }
      // Drag — persist the new split position
      try {
        window.localStorage.setItem(
          DESKTOP_SPLIT_STORAGE_KEY,
          String(desktopSplitPercentRef.current),
        )
      } catch {
        /* ignore */
      }
    }

    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', stopDrag)
    return () => {
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', stopDrag)
    }
  }, [])

  const persistMobilePane = useCallback((pane: 'chat' | 'editor') => {
    setMobilePane(pane)
    try {
      window.localStorage.setItem(MOBILE_PANE_STORAGE_KEY, pane)
    } catch {
      /* ignore */
    }
  }, [])

  const toggleChat = useCallback((next?: boolean) => {
    setShowChat((prev) => {
      const value = next ?? !prev
      // Block collapse when editor is already collapsed — both-panels-collapsed is a dead state
      if (!value && !showEditorRef.current) return prev
      try {
        window.localStorage.setItem(SHOW_CHAT_STORAGE_KEY, String(value))
      } catch {
        /* ignore */
      }
      return value
    })
  }, [])

  const toggleEditor = useCallback((next?: boolean) => {
    setShowEditor((prev) => {
      const value = next ?? !prev
      // Block collapse when chat is already collapsed — both-panels-collapsed is a dead state
      if (!value && !showChatRef.current) return prev
      try {
        window.localStorage.setItem(SHOW_EDITOR_STORAGE_KEY, String(value))
      } catch {
        /* ignore */
      }
      return value
    })
  }, [])

  return {
    desktopSplitPercent,
    setDesktopSplitPercent,
    mobileSplitPercent,
    setMobileSplitPercent,
    isDesktop,
    mobilePane,
    setMobilePane: persistMobilePane,
    showChat,
    setShowChat,
    toggleChat,
    showEditor,
    setShowEditor,
    toggleEditor,
    splitContainerRef,
    splitHandleRef,
    dragStartRef,
    desktopSplitPercentRef,
    mobileSplitPercentRef,
  }
}
