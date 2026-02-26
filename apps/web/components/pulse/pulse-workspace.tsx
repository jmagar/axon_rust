'use client'

import { useCallback, useEffect, useRef, useState } from 'react'
import { useWsMessages } from '@/hooks/use-ws-messages'
import type { ValidationResult } from '@/lib/pulse/doc-ops'
import { validateDocOperations } from '@/lib/pulse/doc-ops'
import { checkPermission } from '@/lib/pulse/permissions'
import type { DocOperation, PulseChatResponse, PulsePermissionLevel } from '@/lib/pulse/types'
import { PulseChatPane } from './pulse-chat-pane'
import { PulseEditorPane } from './pulse-editor-pane'
import { PulseOpConfirmation } from './pulse-op-confirmation'
import { PulseToolbar } from './pulse-toolbar'

export interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
  citations?: PulseChatResponse['citations']
  operations?: PulseChatResponse['operations']
}

export function PulseWorkspace() {
  const { workspacePrompt, workspacePromptVersion } = useWsMessages()
  const [permissionLevel, setPermissionLevel] = useState<PulsePermissionLevel>('training-wheels')
  const [documentMarkdown, setDocumentMarkdown] = useState('')
  const [chatHistory, setChatHistory] = useState<ChatMessage[]>([])
  const [isChatLoading, setIsChatLoading] = useState(false)
  const [documentTitle, setDocumentTitle] = useState('Untitled')
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const [pendingOps, setPendingOps] = useState<DocOperation[] | null>(null)
  const [pendingValidation, setPendingValidation] = useState<ValidationResult | null>(null)
  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const autosaveAbortRef = useRef<AbortController | null>(null)
  const lastSavedSnapshotRef = useRef('')
  const lastHandledPromptVersionRef = useRef(0)
  const chatHistoryRef = useRef<ChatMessage[]>([])

  useEffect(() => {
    chatHistoryRef.current = chatHistory
  }, [chatHistory])

  const applyOperations = useCallback((ops: DocOperation[]) => {
    setDocumentMarkdown((prev) => {
      let next = prev
      for (const op of ops) {
        switch (op.type) {
          case 'replace_document':
            next = op.markdown
            break
          case 'append_markdown':
            next = `${next}\n\n${op.markdown}`
            break
          case 'insert_section':
            next =
              op.position === 'top'
                ? `## ${op.heading}\n\n${op.markdown}\n\n${next}`
                : `${next}\n\n## ${op.heading}\n\n${op.markdown}`
            break
        }
      }
      return next
    })
  }, [])

  useEffect(() => {
    if (workspacePromptVersion === 0) {
      lastHandledPromptVersionRef.current = 0
      return
    }
    if (!workspacePrompt) return
    if (workspacePromptVersion <= lastHandledPromptVersionRef.current) return
    lastHandledPromptVersionRef.current = workspacePromptVersion

    const prompt = workspacePrompt
    setChatHistory((prev) => [...prev, { role: 'user', content: prompt }])
    setIsChatLoading(true)
    const conversationHistory = chatHistoryRef.current.map((m) => ({
      role: m.role,
      content: m.content,
    }))

    void (async () => {
      try {
        const response = await fetch('/api/pulse/chat', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            prompt,
            documentMarkdown,
            selectedCollections: ['pulse', 'cortex'],
            conversationHistory,
            permissionLevel,
          }),
        })
        if (!response.ok) {
          const errorBody = await response.text()
          let detail = ''
          if (errorBody) {
            try {
              const parsed = JSON.parse(errorBody) as { error?: unknown; message?: unknown }
              detail =
                typeof parsed.error === 'string'
                  ? parsed.error
                  : typeof parsed.message === 'string'
                    ? parsed.message
                    : errorBody
            } catch {
              detail = errorBody
            }
          }
          const suffix = detail ? `: ${detail}` : ''
          throw new Error(`Pulse chat failed (${response.status})${suffix}`)
        }

        const data = (await response.json()) as PulseChatResponse
        setChatHistory((prev) => [
          ...prev,
          {
            role: 'assistant',
            content: data.text,
            citations: data.citations,
            operations: data.operations,
          },
        ])

        if (data.operations.length > 0) {
          const permission = checkPermission(permissionLevel, data.operations, {
            isCurrentDoc: true,
            currentDocMarkdown: documentMarkdown,
          })

          if (permission.allowed && !permission.requiresConfirmation) {
            applyOperations(data.operations)
          } else if (permission.allowed && permission.requiresConfirmation) {
            const validation = validateDocOperations(data.operations, documentMarkdown)
            setPendingOps(data.operations)
            setPendingValidation(validation)
          }
        }
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : 'Unknown error'
        setChatHistory((prev) => [...prev, { role: 'assistant', content: `Error: ${message}` }])
      } finally {
        setIsChatLoading(false)
      }
    })()
  }, [
    workspacePromptVersion,
    workspacePrompt,
    documentMarkdown,
    permissionLevel,
    applyOperations,
  ])

  useEffect(() => {
    if (!documentMarkdown || !documentTitle) return

    if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    const snapshot = `${documentTitle}\n---\n${documentMarkdown}`
    if (snapshot === lastSavedSnapshotRef.current) return
    autosaveTimerRef.current = setTimeout(() => {
      void (async () => {
        if (autosaveAbortRef.current) {
          autosaveAbortRef.current.abort()
        }
        const controller = new AbortController()
        autosaveAbortRef.current = controller
        try {
          setSaveStatus('saving')
          const response = await fetch('/api/pulse/save', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            signal: controller.signal,
            body: JSON.stringify({
              title: documentTitle,
              markdown: documentMarkdown,
              embed: true,
            }),
          })
          if (response.ok) {
            lastSavedSnapshotRef.current = snapshot
            setSaveStatus('saved')
          } else {
            setSaveStatus('error')
          }
          setTimeout(() => setSaveStatus('idle'), 2000)
        } catch (error: unknown) {
          if (error instanceof Error && error.name === 'AbortError') return
          setSaveStatus('error')
        } finally {
          if (autosaveAbortRef.current === controller) {
            autosaveAbortRef.current = null
          }
        }
      })()
    }, 1500)

    return () => {
      if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    }
  }, [documentMarkdown, documentTitle])

  useEffect(() => {
    return () => {
      if (autosaveAbortRef.current) {
        autosaveAbortRef.current.abort()
        autosaveAbortRef.current = null
      }
    }
  }, [])

  return (
    <div className="mt-3 flex flex-col gap-2">
      <PulseToolbar
        title={documentTitle}
        onTitleChange={setDocumentTitle}
        permissionLevel={permissionLevel}
        onPermissionChange={setPermissionLevel}
        saveStatus={saveStatus}
      />
      <div className="flex gap-3" style={{ minHeight: '60vh' }}>
        <div className="flex-[3] overflow-hidden rounded-xl border border-[rgba(255,135,175,0.1)] bg-[rgba(10,18,35,0.4)]">
          <PulseEditorPane markdown={documentMarkdown} onMarkdownChange={setDocumentMarkdown} />
        </div>
        <div className="flex-[2] overflow-hidden rounded-xl border border-[rgba(255,135,175,0.1)] bg-[rgba(10,18,35,0.4)]">
          <PulseChatPane messages={chatHistory} isLoading={isChatLoading} />
          {pendingOps && pendingValidation && (
            <div className="p-3">
              <PulseOpConfirmation
                operations={pendingOps}
                validation={pendingValidation}
                onConfirm={() => {
                  applyOperations(pendingOps)
                  setPendingOps(null)
                  setPendingValidation(null)
                }}
                onReject={() => {
                  setPendingOps(null)
                  setPendingValidation(null)
                }}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
