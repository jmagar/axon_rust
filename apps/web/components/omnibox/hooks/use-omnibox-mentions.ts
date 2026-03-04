'use client'

import { useCallback, useMemo, useState } from 'react'
import { apiFetch } from '@/lib/api-fetch'
import type { LocalDocFile, MentionKind } from '@/lib/omnibox'
import {
  extractActiveMention,
  extractMentionLabels,
  getMentionKind,
  replaceActiveMention,
} from '@/lib/omnibox'
import type { ModeDefinition } from '@/lib/ws-protocol'

interface UseOmniboxMentionsInput {
  input: string
  setInput: (value: string) => void
}

export function useOmniboxMentions({ input, setInput }: UseOmniboxMentionsInput) {
  const [mentionSuggestions, setMentionSuggestions] = useState<ModeDefinition[]>([])
  const [fileSuggestions, setFileSuggestions] = useState<LocalDocFile[]>([])
  const [mentionSelectionIndex, setMentionSelectionIndex] = useState(0)
  const [modeAppliedLabel, setModeAppliedLabel] = useState<string | null>(null)
  const [localDocFiles, setLocalDocFiles] = useState<LocalDocFile[]>([])
  const [fileContextMentions, setFileContextMentions] = useState<Record<string, LocalDocFile>>({})
  const [recentFileSelections, setRecentFileSelections] = useState<Record<string, number>>({})

  const activeMentionToken = useMemo(() => extractActiveMention(input), [input])
  const mentionKind: MentionKind = useMemo(
    () => getMentionKind(input, activeMentionToken),
    [input, activeMentionToken],
  )
  const activeSuggestions = mentionKind === 'mode' ? mentionSuggestions : fileSuggestions

  const buildInputWithFileContext = useCallback(
    async (rawInput: string) => {
      const mentionLabels = extractMentionLabels(rawInput)
      const matchingFiles = mentionLabels
        .map((label) => fileContextMentions[label.toLowerCase()])
        .filter((file): file is LocalDocFile => Boolean(file))
        .slice(0, 3)

      if (matchingFiles.length === 0) {
        return { enrichedInput: rawInput.trim(), contextFileLabels: [] as string[] }
      }

      const contextBlocks = await Promise.all(
        matchingFiles.map(async (file) => {
          try {
            const res = await apiFetch(`/api/omnibox/files?id=${encodeURIComponent(file.id)}`)
            if (!res.ok) return null
            const data = (await res.json()) as { file?: { content?: string; label?: string } }
            const content = data.file?.content?.trim()
            if (!content) return null
            const label = data.file?.label ?? file.label
            return `### ${label}\n${content.slice(0, 2400)}`
          } catch {
            return null
          }
        }),
      )

      const usableBlocks = contextBlocks.filter((block): block is string => Boolean(block))
      if (usableBlocks.length === 0) {
        return { enrichedInput: rawInput.trim(), contextFileLabels: [] as string[] }
      }

      const contextSection = `\n\nLocal file context:\n${usableBlocks.join('\n\n---\n\n')}`
      return {
        enrichedInput: `${rawInput.trim()}${contextSection}`,
        contextFileLabels: matchingFiles.map((file) => file.label),
      }
    },
    [fileContextMentions],
  )

  const applyFileMentionCandidate = useCallback(
    (candidate: LocalDocFile) => {
      if (!activeMentionToken) return false
      const nextInput = replaceActiveMention(input, activeMentionToken, `@${candidate.label} `)
      setInput(nextInput)
      setFileSuggestions([])
      setMentionSuggestions([])
      setMentionSelectionIndex(0)
      setFileContextMentions((prev) => ({
        ...prev,
        [candidate.label.toLowerCase()]: candidate,
      }))
      setRecentFileSelections((prev) => ({
        ...prev,
        [candidate.id]: Date.now(),
      }))
      return true
    },
    [activeMentionToken, input, setInput],
  )

  const removeFileContextMention = useCallback((label: string) => {
    setFileContextMentions((prev) => {
      const next = { ...prev }
      delete next[label]
      return next
    })
  }, [])

  return {
    mentionSuggestions,
    fileSuggestions,
    mentionSelectionIndex,
    modeAppliedLabel,
    localDocFiles,
    fileContextMentions,
    recentFileSelections,
    activeMentionToken,
    mentionKind,
    activeSuggestions,
    setMentionSuggestions,
    setFileSuggestions,
    setMentionSelectionIndex,
    setModeAppliedLabel,
    setLocalDocFiles,
    buildInputWithFileContext,
    applyFileMentionCandidate,
    removeFileContextMention,
  }
}
