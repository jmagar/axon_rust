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
  const { workspacePrompt } = useWsMessages()
  const [permissionLevel, setPermissionLevel] = useState<PulsePermissionLevel>('training-wheels')
  const [documentMarkdown, setDocumentMarkdown] = useState('')
  const [chatHistory, setChatHistory] = useState<ChatMessage[]>([])
  const [isChatLoading, setIsChatLoading] = useState(false)
  const [documentTitle, setDocumentTitle] = useState('Untitled')
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle')
  const [pendingOps, setPendingOps] = useState<DocOperation[] | null>(null)
  const [pendingValidation, setPendingValidation] = useState<ValidationResult | null>(null)
  const autosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const lastPromptRef = useRef<string | null>(null)

  const applyOperations = useCallback((ops: DocOperation[]) => {
    for (const op of ops) {
      switch (op.type) {
        case 'replace_document':
          setDocumentMarkdown(op.markdown)
          break
        case 'append_markdown':
          setDocumentMarkdown((prev) => `${prev}\n\n${op.markdown}`)
          break
        case 'insert_section':
          setDocumentMarkdown((prev) =>
            op.position === 'top'
              ? `## ${op.heading}\n\n${op.markdown}\n\n${prev}`
              : `${prev}\n\n## ${op.heading}\n\n${op.markdown}`,
          )
          break
      }
    }
  }, [])

  useEffect(() => {
    if (!workspacePrompt || workspacePrompt === lastPromptRef.current) return
    lastPromptRef.current = workspacePrompt

    const prompt = workspacePrompt
    setChatHistory((prev) => [...prev, { role: 'user', content: prompt }])
    setIsChatLoading(true)

    fetch('/api/pulse/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        prompt,
        documentMarkdown,
        selectedCollections: ['pulse', 'cortex'],
        conversationHistory: chatHistory.map((m) => ({ role: m.role, content: m.content })),
        permissionLevel,
      }),
    })
      .then((res) => res.json())
      .then((data: PulseChatResponse) => {
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
          const perm = checkPermission(permissionLevel, data.operations, {
            isCurrentDoc: true,
            currentDocMarkdown: documentMarkdown,
          })

          if (perm.allowed && !perm.requiresConfirmation) {
            applyOperations(data.operations)
          } else if (perm.allowed && perm.requiresConfirmation) {
            const validation = validateDocOperations(data.operations, documentMarkdown)
            setPendingOps(data.operations)
            setPendingValidation(validation)
          }
        }
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : 'Unknown error'
        setChatHistory((prev) => [...prev, { role: 'assistant', content: `Error: ${message}` }])
      })
      .finally(() => setIsChatLoading(false))
  }, [workspacePrompt, documentMarkdown, chatHistory, permissionLevel, applyOperations])

  useEffect(() => {
    if (!documentMarkdown || !documentTitle) return

    if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    autosaveTimerRef.current = setTimeout(() => {
      setSaveStatus('saving')
      fetch('/api/pulse/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: documentTitle, markdown: documentMarkdown, embed: true }),
      })
        .then((res) => {
          setSaveStatus(res.ok ? 'saved' : 'error')
          setTimeout(() => setSaveStatus('idle'), 2000)
        })
        .catch(() => setSaveStatus('error'))
    }, 1500)

    return () => {
      if (autosaveTimerRef.current) clearTimeout(autosaveTimerRef.current)
    }
  }, [documentMarkdown, documentTitle])

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
        <div className="flex-[3] overflow-hidden rounded-xl border border-[rgba(175,215,255,0.1)] bg-[rgba(10,18,35,0.4)]">
          <PulseEditorPane markdown={documentMarkdown} onMarkdownChange={setDocumentMarkdown} />
        </div>
        <div className="flex-[2] overflow-hidden rounded-xl border border-[rgba(175,215,255,0.1)] bg-[rgba(10,18,35,0.4)]">
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
