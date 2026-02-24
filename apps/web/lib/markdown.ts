import { deserializeMd, MarkdownPlugin, remarkMdx, remarkMention } from '@platejs/markdown'
import type { Descendant } from 'platejs'
import { createSlateEditor } from 'platejs'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'

/** Empty paragraph node used as fallback for blank input. */
const EMPTY_VALUE: Descendant[] = [{ type: 'p', children: [{ text: '' }] }]

/**
 * Singleton editor instance used exclusively for markdown deserialization.
 * Configured with GFM (tables, strikethrough), math, MDX, and mention support
 * to match the editor plugins in `components/editor/plugins/markdown-kit.tsx`.
 */
const markdownEditor = createSlateEditor({
  plugins: [
    MarkdownPlugin.configure({
      options: {
        remarkPlugins: [remarkMath, remarkGfm, remarkMdx, remarkMention],
      },
    }),
  ],
})

/**
 * Convert a markdown string to Plate editor value (array of Descendant nodes).
 *
 * Uses `@platejs/markdown` `deserializeMd` with a pre-configured editor that
 * supports GFM tables, math blocks, MDX, and mentions. Returns a minimal
 * empty paragraph if the input is blank.
 *
 * @param md - Raw markdown string (e.g. from CLI command output)
 * @returns Array of Plate Descendant nodes ready for `editor.children` or `<Plate value={...}>`
 */
export function markdownToPlateNodes(md: string): Descendant[] {
  if (!md.trim()) {
    return EMPTY_VALUE
  }

  return deserializeMd(markdownEditor, md)
}
