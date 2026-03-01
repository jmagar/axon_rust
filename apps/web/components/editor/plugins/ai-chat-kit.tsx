'use client'

import { AIChatPlugin, AIPlugin } from '@platejs/ai/react'
import type { usePlateEditor } from 'platejs/react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { AIAnchorElement, AILeaf } from '@/components/ui/ai-node'

type ChatStatus = 'idle' | 'submitted' | 'streaming' | 'error'

interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  parts: Array<{ type: 'text'; text: string }>
}

interface ChatHelpers {
  status: ChatStatus
  messages: ChatMessage[]
  sendMessage: (message: { text: string }, options?: { body?: Record<string, unknown> }) => void
}

/** Custom chat adapter that streams from /api/ai/chat into the AIChatPlugin. */
export function useAxonAIChat(): ChatHelpers {
  const [status, setStatus] = useState<ChatStatus>('idle')
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const abortRef = useRef<AbortController | null>(null)

  const sendMessage = useCallback(
    async (message: { text: string }, options?: { body?: Record<string, unknown> }) => {
      abortRef.current?.abort()
      const ctrl = new AbortController()
      abortRef.current = ctrl

      const msgId = crypto.randomUUID()
      setStatus('submitted')
      setMessages((prev) => [
        ...prev,
        { id: msgId, role: 'assistant', parts: [{ type: 'text', text: '' }] },
      ])

      try {
        const res = await fetch('/api/ai/chat', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ prompt: message.text, ...options?.body }),
          signal: ctrl.signal,
        })

        if (!res.ok || !res.body) {
          setStatus('error')
          return
        }

        setStatus('streaming')
        const reader = res.body.getReader()
        const decoder = new TextDecoder()
        let accumulated = ''

        while (true) {
          const { value, done } = await reader.read()
          if (done) break

          const chunk = decoder.decode(value, { stream: true })
          for (const rawLine of chunk.split('\n')) {
            const line = rawLine.trim()
            if (!line.startsWith('data:')) continue
            const payload = line.slice(5).trim()
            if (payload === '[DONE]') break
            try {
              const delta = (
                JSON.parse(payload) as { choices?: Array<{ delta?: { content?: string } }> }
              ).choices?.[0]?.delta?.content
              if (delta) {
                accumulated += delta
                setMessages((prev) => {
                  const msgs = [...prev]
                  const last = msgs[msgs.length - 1]
                  if (!last) return prev
                  msgs[msgs.length - 1] = {
                    ...last,
                    parts: [{ type: 'text', text: accumulated }],
                  }
                  return msgs
                })
              }
            } catch {
              // Ignore malformed SSE lines.
            }
          }
        }

        setStatus('idle')
      } catch (err) {
        if ((err as Error).name !== 'AbortError') setStatus('error')
      }
    },
    [],
  )

  return useMemo(() => ({ status, messages, sendMessage }), [status, messages, sendMessage])
}

/** Wire the chat adapter into AIChatPlugin after the editor mounts. */
export function useAIChatSetup(editor: ReturnType<typeof usePlateEditor>) {
  const chat = useAxonAIChat()

  useEffect(() => {
    if (!editor) return
    // biome-ignore lint/suspicious/noExplicitAny: custom chat adapter, types differ from platejs expected shape
    editor.setOption(AIChatPlugin, 'chat', chat as any)
  }, [editor, chat])
}

export const AIChatKit = [
  AIPlugin.configure({
    render: { node: AIAnchorElement },
  }),
  AIChatPlugin.configure({
    render: { node: AILeaf },
    options: {
      mode: 'insert' as const,
    },
  }),
]
