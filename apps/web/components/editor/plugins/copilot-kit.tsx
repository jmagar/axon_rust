'use client'

import { CopilotPlugin } from '@platejs/ai/react'
import { serializeMd, stripMarkdown } from '@platejs/markdown'
import type { TElement } from 'platejs'

import { GhostText } from '@/components/ui/ghost-text'

import { AIChatKit } from './ai-chat-kit'
import { BasicBlocksKit } from './basic-blocks-kit'
import { BasicMarksKit } from './basic-marks-kit'
import { CalloutKit } from './callout-kit'
import { CommentKit } from './comment-kit'
import { DiscussionKit } from './discussion-kit'
import { DndKit } from './dnd-kit'
import { ExtendedNodesKit } from './extended-nodes-kit'
import { MarkdownKit } from './markdown-kit'
import { SelectionKit } from './selection-kit'
import { SlashKit } from './slash-kit'
import { SuggestionKit } from './suggestion-kit'
import { TocKit } from './toc-kit'
import { ToggleKit } from './toggle-kit'

type CopilotNdjsonEvent =
  | { type: 'start' }
  | { type: 'delta'; delta?: string }
  | { type: 'done'; completion?: string }
  | { type: 'error'; error?: string }

const NDJSON_CONTENT_TYPE = 'application/x-ndjson'

const copilotStreamingFetch: typeof fetch = async (input, init) => {
  const headers = new Headers(init?.headers)
  headers.set('accept', NDJSON_CONTENT_TYPE)
  headers.set('x-copilot-stream', '1')

  const response = await fetch(input, { ...init, headers })
  const contentType = response.headers.get('content-type')?.toLowerCase() ?? ''

  if (!contentType.includes(NDJSON_CONTENT_TYPE) || !response.body) {
    return response
  }

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  const encoder = new TextEncoder()

  const transformed = new ReadableStream<Uint8Array>({
    async start(controller) {
      let remainder = ''
      let emitted = false
      let finalCompletion = ''

      try {
        while (true) {
          const { value, done } = await reader.read()
          if (done) break

          const chunk = decoder.decode(value, { stream: true })
          const parsed = parseCopilotNdjsonChunk(chunk, remainder)
          remainder = parsed.remainder

          for (const event of parsed.events) {
            if (
              event.type === 'delta' &&
              typeof event.delta === 'string' &&
              event.delta.length > 0
            ) {
              emitted = true
              controller.enqueue(encoder.encode(event.delta))
            }
            if (event.type === 'done' && typeof event.completion === 'string') {
              finalCompletion = event.completion
            }
          }
        }

        if (!emitted && finalCompletion) {
          controller.enqueue(encoder.encode(finalCompletion))
        }
      } finally {
        controller.close()
      }
    },
  })

  return new Response(transformed, {
    status: response.status,
    statusText: response.statusText,
    headers: {
      'Content-Type': 'text/plain; charset=utf-8',
      'Cache-Control': 'no-store',
    },
  })
}

function parseCopilotNdjsonChunk(
  chunk: string,
  remainder: string,
): { events: CopilotNdjsonEvent[]; remainder: string } {
  const combined = remainder + chunk
  const lines = combined.split('\n')
  const nextRemainder = lines.pop() ?? ''
  const events: CopilotNdjsonEvent[] = []

  for (const rawLine of lines) {
    const line = rawLine.trim()
    if (!line) continue
    try {
      const parsed = JSON.parse(line) as CopilotNdjsonEvent
      if (parsed && typeof parsed === 'object' && typeof parsed.type === 'string') {
        events.push(parsed)
      }
    } catch {
      // Ignore malformed NDJSON lines from interrupted chunks.
    }
  }

  return { events, remainder: nextRemainder }
}

export const CopilotKit = [
  ...BasicBlocksKit,
  ...BasicMarksKit,
  ...MarkdownKit,
  ...ExtendedNodesKit,
  ...AIChatKit,
  ...SlashKit,
  ...DndKit,
  ...CalloutKit,
  ...ToggleKit,
  ...TocKit,
  ...SelectionKit,
  ...DiscussionKit,
  ...CommentKit,
  ...SuggestionKit,
  CopilotPlugin.configure(({ api }) => ({
    options: {
      completeOptions: {
        api: '/api/ai/copilot',
        fetch: copilotStreamingFetch,
        body: {
          system: `You are an advanced AI writing assistant, similar to VSCode Copilot but for general text. Your task is to predict and generate the next part of the text based on the given context.

  Rules:
  - Continue the text naturally up to the next punctuation mark (., ,, ;, :, ?, or !).
  - Maintain style and tone. Don't repeat given text.
  - For unclear context, provide the most likely continuation.
  - Handle code snippets, lists, or structured text if needed.
  - Don't include """ in your response.
  - CRITICAL: Always end with a punctuation mark.
  - CRITICAL: Avoid starting a new block. Do not use block formatting like >, #, 1., 2., -, etc. The suggestion should continue in the same block as the context.
  - If no context is provided or you can't generate a continuation, return "0" without explanation.`,
        },
        onError: (error) => {
          console.error('[Copilot] API error:', error)
          api.copilot.setBlockSuggestion({ text: '' })
        },
        onFinish: (_, completion) => {
          if (completion === '0') return

          api.copilot.setBlockSuggestion({
            text: stripMarkdown(completion),
          })
        },
      },
      debounceDelay: 500,
      renderGhostText: GhostText,
      getPrompt: ({ editor }) => {
        const contextEntry = editor.api.block({ highest: true })

        if (!contextEntry) return ''

        const prompt = serializeMd(editor, {
          value: [contextEntry[0] as TElement],
        })

        return `Continue the text up to the next punctuation mark:
  """
  ${prompt}
  """`
      },
    },
    shortcuts: {
      accept: {
        keys: 'tab',
      },
      acceptNextWord: {
        keys: 'mod+right',
      },
      reject: {
        keys: 'escape',
      },
      triggerSuggestion: {
        keys: 'ctrl+space',
      },
    },
  })),
]
