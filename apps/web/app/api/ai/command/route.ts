import { createGateway } from '@ai-sdk/gateway'
import {
  createUIMessageStream,
  createUIMessageStreamResponse,
  generateText,
  type LanguageModel,
  streamText,
  tool,
  type UIMessageStreamWriter,
} from 'ai'
import type { NextRequest } from 'next/server'
import { NextResponse } from 'next/server'
import { createSlateEditor, nanoid, type SlateEditor } from 'platejs'
import { z } from 'zod'
import { BaseEditorKit } from '@/components/editor/editor-base-kit'
import type { ChatMessage, ToolName } from '@/components/editor/use-chat'
import { markdownJoinerTransform } from '@/lib/markdown-joiner-transform'

import {
  buildEditTableMultiCellPrompt,
  getChooseToolPrompt,
  getCommentPrompt,
  getEditPrompt,
  getGeneratePrompt,
} from './prompt'

export async function POST(req: NextRequest) {
  try {
    const body = await req.json()
    const {
      apiKey: key,
      ctx,
      messages: messagesRaw,
      model,
    } = body as {
      apiKey?: string
      ctx?: {
        children?: SlateEditor['children']
        selection?: SlateEditor['selection']
        toolName?: string
      }
      messages?: ChatMessage[]
      model?: string
    }

    if (!ctx || typeof ctx !== 'object' || !('children' in ctx) || !('selection' in ctx)) {
      return NextResponse.json({ error: 'Missing required ctx payload.' }, { status: 400 })
    }
    if (!Array.isArray(messagesRaw)) {
      return NextResponse.json({ error: 'Missing or invalid messages payload.' }, { status: 400 })
    }

    const { children, selection, toolName: toolNameParam } = ctx

    const editor = createSlateEditor({
      plugins: BaseEditorKit,
      selection,
      value: children,
    })

    const apiKey = key || process.env.AI_GATEWAY_API_KEY
    if (!apiKey) {
      return NextResponse.json({ error: 'Missing AI Gateway API key.' }, { status: 401 })
    }

    const isSelecting = editor.api.isExpanded()
    const gatewayProvider = createGateway({ apiKey })
    const enumOptions = isSelecting ? ['generate', 'edit', 'comment'] : ['generate', 'comment']
    const validToolNames = new Set<ToolName>(['generate', 'edit', 'comment'])
    let validatedToolName =
      typeof toolNameParam === 'string' && validToolNames.has(toolNameParam as ToolName)
        ? (toolNameParam as ToolName)
        : undefined

    const stream = createUIMessageStream<ChatMessage>({
      execute: async ({ writer }) => {
        let toolName = validatedToolName

        if (!toolName) {
          const prompt = getChooseToolPrompt({
            isSelecting,
            messages: messagesRaw,
          })

          const modelId = model || 'google/gemini-2.5-flash'

          const { text: rawToolName } = await generateText({
            model: gatewayProvider(modelId),
            prompt,
            system: `Respond with exactly one word from: ${enumOptions.join(', ')}. No other text.`,
          })
          const AIToolName = (
            enumOptions.includes(rawToolName.trim()) ? rawToolName.trim() : enumOptions[0]
          ) as ToolName

          writer.write({
            data: AIToolName as ToolName,
            type: 'data-toolName',
          })

          toolName = AIToolName
          validatedToolName = AIToolName
        } else {
          writer.write({
            data: toolName,
            type: 'data-toolName',
          })
        }

        const stream = streamText({
          experimental_transform: markdownJoinerTransform(),
          model: gatewayProvider(model || 'openai/gpt-4o-mini'),
          // Not used
          prompt: '',
          tools: {
            comment: getCommentTool(editor, {
              messagesRaw,
              model: gatewayProvider(model || 'google/gemini-2.5-flash'),
              writer,
            }),
            table: getTableTool(editor, {
              messagesRaw,
              model: gatewayProvider(model || 'google/gemini-2.5-flash'),
              writer,
            }),
          },
          prepareStep: async (step) => {
            if (toolName === 'comment') {
              return {
                ...step,
                toolChoice: { toolName: 'comment', type: 'tool' },
              }
            }

            if (toolName === 'edit') {
              const [editPrompt, editType] = getEditPrompt(editor, {
                isSelecting,
                messages: messagesRaw,
              })

              // Table editing uses the table tool
              if (editType === 'table') {
                return {
                  ...step,
                  toolChoice: { toolName: 'table', type: 'tool' },
                }
              }

              return {
                ...step,
                activeTools: [],
                model:
                  editType === 'selection'
                    ? //The selection task is more challenging, so we chose to use Gemini 2.5 Flash.
                      gatewayProvider(model || 'google/gemini-2.5-flash')
                    : gatewayProvider(model || 'openai/gpt-4o-mini'),
                messages: [
                  {
                    content: editPrompt,
                    role: 'user',
                  },
                ],
              }
            }

            if (toolName === 'generate') {
              const generatePrompt = getGeneratePrompt(editor, {
                isSelecting,
                messages: messagesRaw,
              })

              return {
                ...step,
                activeTools: [],
                messages: [
                  {
                    content: generatePrompt,
                    role: 'user',
                  },
                ],
                model: gatewayProvider(model || 'openai/gpt-4o-mini'),
              }
            }
          },
        })

        writer.merge(stream.toUIMessageStream({ sendFinish: false }))
      },
    })

    return createUIMessageStreamResponse({ stream })
  } catch {
    return NextResponse.json({ error: 'Failed to process AI request' }, { status: 500 })
  }
}

const getCommentTool = (
  editor: SlateEditor,
  {
    messagesRaw,
    model,
    writer,
  }: {
    messagesRaw: ChatMessage[]
    model: LanguageModel
    writer: UIMessageStreamWriter<ChatMessage>
  },
) =>
  tool({
    description: 'Comment on the content',
    inputSchema: z.object({}),
    execute: async () => {
      const commentDataId = nanoid()
      try {
        const { text } = await generateText({
          model,
          prompt: getCommentPrompt(editor, { messages: messagesRaw }),
          system:
            'Return a JSON array of comment objects. Each object must have: blockId (string), comment (string), content (string). Only return the JSON array, no other text.',
        })
        const comments = JSON.parse(text) as Array<{
          blockId: string
          comment: string
          content: string
        }>
        for (const comment of comments) {
          writer.write({
            id: commentDataId,
            data: { comment, status: 'streaming' },
            type: 'data-comment',
          })
        }
      } catch {
        // Ignore tool and JSON errors — fall through to finished signal.
      } finally {
        writer.write({
          id: nanoid(),
          data: { comment: null, status: 'finished' },
          type: 'data-comment',
        })
      }
    },
  })

const getTableTool = (
  editor: SlateEditor,
  {
    messagesRaw,
    model,
    writer,
  }: {
    messagesRaw: ChatMessage[]
    model: LanguageModel
    writer: UIMessageStreamWriter<ChatMessage>
  },
) =>
  tool({
    description: 'Edit table cells',
    inputSchema: z.object({}),
    execute: async () => {
      try {
        const { text } = await generateText({
          model,
          prompt: buildEditTableMultiCellPrompt(editor, messagesRaw),
          system:
            'Return a JSON array of cell update objects. Each object must have: id (string cell id), content (string new content). Only return the JSON array, no other text.',
        })
        const updates = JSON.parse(text) as Array<{ id: string; content: string }>
        for (const cellUpdate of updates) {
          writer.write({
            id: nanoid(),
            data: { cellUpdate, status: 'streaming' },
            type: 'data-table',
          })
        }
      } catch {
        // Ignore tool and JSON errors — fall through to finished signal.
      } finally {
        writer.write({
          id: nanoid(),
          data: { cellUpdate: null, status: 'finished' },
          type: 'data-table',
        })
      }
    },
  })
