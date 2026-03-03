'use client'

import { type UseChatHelpers, useChat as useBaseChat } from '@ai-sdk/react'
import { withAIBatch } from '@platejs/ai'
import { AIChatPlugin, aiCommentToRange, applyTableCellSuggestion } from '@platejs/ai/react'
import { getCommentKey, getTransientCommentKey } from '@platejs/comment'
import { deserializeMd } from '@platejs/markdown'
import { BlockSelectionPlugin } from '@platejs/selection/react'
import { DefaultChatTransport, type UIMessage } from 'ai'
import { KEYS, NodeApi, nanoid, TextApi, type TNode } from 'platejs'
import { useEditorRef, usePluginOption } from 'platejs/react'
import * as React from 'react'
import { aiChatPlugin } from '@/components/editor/plugins/ai-kit'
import { discussionPlugin } from './plugins/discussion-kit'
import { fakeStreamText } from './use-chat-fake-stream'

export type ToolName = 'comment' | 'edit' | 'generate'

interface ChatRequestBody {
  ctx?: {
    children?: Array<{ type?: string }>
    selection?: {
      anchor?: { path?: number[] }
      focus?: { path?: number[] }
    }
  }
  messages: Array<{
    parts: Array<{ text?: string; type?: string }>
  }>
}

export interface TComment {
  comment: {
    blockId: string
    comment: string
    content: string
  } | null
  status: 'finished' | 'streaming'
}

export interface TTableCellUpdate {
  cellUpdate: {
    content: string
    id: string
  } | null
  status: 'finished' | 'streaming'
}

export interface MessageDataPart {
  [key: string]: unknown
  toolName: ToolName
  comment?: TComment
  table?: TTableCellUpdate
}

export type Chat = UseChatHelpers<ChatMessage>

export type ChatMessage = UIMessage<unknown, MessageDataPart>

export const useChat = () => {
  const editor = useEditorRef()
  const options = usePluginOption(aiChatPlugin, 'chatOptions')

  const abortControllerRef = React.useRef<AbortController | null>(null)
  const _abortFakeStream = () => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
      abortControllerRef.current = null
    }
  }

  const baseChat = useBaseChat<ChatMessage>({
    id: 'editor',
    transport: new DefaultChatTransport({
      api: options.api || '/api/ai/command',
      fetch: (async (input, init) => {
        const bodyOptions = editor.getOptions(aiChatPlugin).chatOptions?.body

        const initBody = JSON.parse(init?.body as string) as ChatRequestBody

        const body = {
          ...initBody,
          ...bodyOptions,
        }
        const shouldUseFakeFallback =
          process.env.NODE_ENV === 'development' &&
          process.env.NEXT_PUBLIC_ENABLE_FAKE_AI_STREAM === 'true'

        try {
          const res = await fetch(input, {
            ...init,
            body: JSON.stringify(body),
          })

          if (res.ok || !shouldUseFakeFallback) {
            return res
          }
        } catch {
          if (!shouldUseFakeFallback) {
            return new Response(JSON.stringify({ error: 'AI request failed' }), {
              status: 500,
              headers: {
                'Content-Type': 'application/json',
              },
            })
          }
        }

        let sample: 'comment' | 'markdown' | 'mdx' | 'table' | null = null

        try {
          const content = body.messages
            .at(-1)
            ?.parts.find((p: { text?: string; type?: string }) => p.type === 'text')?.text

          if (content?.includes('Generate a markdown sample')) {
            sample = 'markdown'
          } else if (content?.includes('Generate a mdx sample')) {
            sample = 'mdx'
          } else if (content?.includes('comment')) {
            sample = 'comment'
          }

          // Detect table editing by checking if multiple table cells are selected
          // Single cell selection should use normal edit flow, only multi-cell uses table tool
          if (!sample) {
            // First check: selectedCells from TablePlugin (cell selection mode)
            const selectedCells = editor.getOption({ key: KEYS.table }, 'selectedCells') || []

            if (selectedCells.length > 1) {
              sample = 'table'
            }
            // Second check: selection range spans multiple cells
            else if (body.ctx?.children && body.ctx?.selection) {
              const { selection, children } = body.ctx
              const anchorPath = selection.anchor?.path
              const focusPath = selection.focus?.path

              if (anchorPath && anchorPath.length >= 3) {
                const rootIndex = anchorPath[0]
                const rootNode = children[rootIndex]

                if (rootNode?.type === 'table') {
                  // Cell path is at index 2 (table -> row -> cell)
                  const anchorCellPath = anchorPath.slice(0, 3).join(',')
                  const focusCellPath = focusPath?.slice(0, 3).join(',')

                  // Only use table mock if anchor and focus are in different cells
                  if (focusCellPath && anchorCellPath !== focusCellPath) {
                    sample = 'table'
                  }
                }
              }
            }
          }
        } catch {
          sample = null
        }

        abortControllerRef.current = new AbortController()
        await new Promise((resolve) => setTimeout(resolve, 400))

        const stream = fakeStreamText({
          editor,
          sample,
          signal: abortControllerRef.current.signal,
        })

        return new Response(stream, {
          headers: {
            Connection: 'keep-alive',
            'Content-Type': 'text/plain',
          },
        })
      }) as typeof fetch,
    }),
    onData(data) {
      if (data.type === 'data-toolName') {
        editor.setOption(AIChatPlugin, 'toolName', data.data as ToolName)
      }

      if (data.type === 'data-table' && data.data) {
        const tableData = data.data as TTableCellUpdate

        const cellUpdate = tableData.cellUpdate
        if (cellUpdate) {
          withAIBatch(editor, () => {
            applyTableCellSuggestion(editor, cellUpdate)
          })
        }

        if (tableData.status === 'finished') {
          const chatSelection = editor.getOption(AIChatPlugin, 'chatSelection')

          if (!chatSelection) return

          editor.tf.setSelection(chatSelection)

          return
        }
      }

      if (data.type === 'data-comment' && data.data) {
        const commentData = data.data as TComment

        if (commentData.comment) {
          const aiComment = commentData.comment
          const range = aiCommentToRange(editor, aiComment)

          if (!range) return console.warn('No range found for AI comment')

          const discussions = editor.getOption(discussionPlugin, 'discussions') || []

          // Generate a new discussion ID
          const discussionId = nanoid()

          // Create a new comment
          const newComment = {
            id: nanoid(),
            contentRich: [{ children: [{ text: aiComment.comment }], type: 'p' }],
            createdAt: new Date(),
            discussionId,
            isEdited: false,
            userId: editor.getOption(discussionPlugin, 'currentUserId'),
          }

          // Create a new discussion
          const newDiscussion = {
            id: discussionId,
            comments: [newComment],
            createdAt: new Date(),
            documentContent: deserializeMd(editor, aiComment.content)
              .map((node: TNode) => NodeApi.string(node))
              .join('\n'),
            isResolved: false,
            userId: editor.getOption(discussionPlugin, 'currentUserId'),
          }

          // Update discussions
          const updatedDiscussions = [...discussions, newDiscussion]
          editor.setOption(discussionPlugin, 'discussions', updatedDiscussions)

          // Apply comment marks to the editor
          editor.tf.withMerging(() => {
            editor.tf.setNodes(
              {
                [getCommentKey(newDiscussion.id)]: true,
                [getTransientCommentKey()]: true,
                [KEYS.comment]: true,
              },
              {
                at: range,
                match: TextApi.isText,
                split: true,
              },
            )
          })
        }

        if (commentData.status === 'finished') {
          editor.getApi(BlockSelectionPlugin).blockSelection.deselect()

          return
        }
      }
    },

    ...options,
  })

  const chat = {
    ...baseChat,
    _abortFakeStream,
  }

  const chatRef = React.useRef(chat)
  chatRef.current = chat

  // biome-ignore lint/correctness/useExhaustiveDependencies: status/messages/error are intentional triggers to re-sync chatRef.current into the plugin
  React.useEffect(() => {
    // biome-ignore lint/suspicious/noExplicitAny: custom adapter shape differs from platejs internal ChatHelpers
    editor.setOption(AIChatPlugin, 'chat', chatRef.current as any)
  }, [editor, baseChat.status, baseChat.messages, baseChat.error])

  return chat
}
