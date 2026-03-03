'use client'

import type React from 'react'
import { useCallback } from 'react'
import type { MentionKind } from '@/lib/omnibox'

interface UseOmniboxKeyboardInput {
  activeSuggestions: unknown[]
  mentionKind: MentionKind
  applyActiveSuggestion: () => boolean
  execute: () => void
  setDropdownOpen: (v: boolean) => void
  setOptionsOpen: (v: boolean) => void
  setMentionSuggestions: (v: never[]) => void
  setFileSuggestions: (v: never[]) => void
  setMentionSelectionIndex: React.Dispatch<React.SetStateAction<number>>
  setInput: (v: string) => void
}

export function useOmniboxKeyboard({
  activeSuggestions,
  mentionKind,
  applyActiveSuggestion,
  execute,
  setDropdownOpen,
  setOptionsOpen,
  setMentionSuggestions,
  setFileSuggestions,
  setMentionSelectionIndex,
  setInput,
}: UseOmniboxKeyboardInput) {
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const hasMentionSelection = activeSuggestions.length > 0 && mentionKind !== 'none'
      if (e.key === 'ArrowDown' && hasMentionSelection) {
        e.preventDefault()
        setMentionSelectionIndex((prev: number) => (prev + 1) % activeSuggestions.length)
        return
      }
      if (e.key === 'ArrowUp' && hasMentionSelection) {
        e.preventDefault()
        setMentionSelectionIndex(
          (prev: number) => (prev - 1 + activeSuggestions.length) % activeSuggestions.length,
        )
        return
      }
      if (e.key === 'Tab' && hasMentionSelection) {
        e.preventDefault()
        applyActiveSuggestion()
        return
      }
      if (e.key === 'Enter') {
        if (hasMentionSelection) {
          e.preventDefault()
          applyActiveSuggestion()
          return
        }
        if ((e.metaKey || e.ctrlKey) && !e.altKey) {
          e.preventDefault()
          execute()
          return
        }
        e.preventDefault()
        execute()
      }
      if (e.key === 'Escape') {
        setDropdownOpen(false)
        setOptionsOpen(false)
        setMentionSuggestions([] as never[])
        setFileSuggestions([] as never[])
        if (mentionKind === 'mode') {
          setInput('')
        }
      }
    },
    [
      activeSuggestions,
      mentionKind,
      applyActiveSuggestion,
      execute,
      setDropdownOpen,
      setOptionsOpen,
      setMentionSuggestions,
      setFileSuggestions,
      setMentionSelectionIndex,
      setInput,
    ],
  )

  return { handleKeyDown }
}
