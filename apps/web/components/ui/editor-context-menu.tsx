'use client'

import { ChevronRight } from 'lucide-react'
import { useEditorRef } from 'platejs/react'
import { ContextMenu } from 'radix-ui'
import type * as React from 'react'

// Shared class strings for menu styling
const contentCls =
  'bg-[rgba(3,7,18,0.92)] border border-[var(--border-standard)] backdrop-blur-md rounded-md shadow-2xl py-1 min-w-[160px] z-50'

const itemCls =
  'flex items-center gap-2 px-3 py-1.5 text-[11px] text-[var(--text-secondary)] cursor-pointer select-none outline-none hover:bg-[var(--surface-elevated)] hover:text-[var(--axon-secondary)] data-[disabled]:opacity-40 data-[disabled]:cursor-not-allowed'

const separatorCls = 'my-1 h-px bg-[var(--border-subtle)]'

const shortcutCls = 'ml-auto text-[10px] text-[var(--text-muted)]'

// ─── Shortcut hint span ───────────────────────────────────────────────────────
function Shortcut({ children }: { children: React.ReactNode }) {
  return <span className={shortcutCls}>{children}</span>
}

// ─── EditorContextMenu ────────────────────────────────────────────────────────
export function EditorContextMenu({ children }: { children: React.ReactNode }) {
  const editor = useEditorRef()

  // Wrap action in rAF so the menu is fully closed and editor focus is
  // restored before the transform fires.
  function withRaf(fn: () => void) {
    return () => {
      requestAnimationFrame(fn)
    }
  }

  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>

      <ContextMenu.Portal>
        <ContextMenu.Content className={contentCls}>
          {/* ── Editing ── */}
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              const text = window.getSelection()?.toString() ?? ''
              if (text)
                navigator.clipboard.writeText(text).catch(() => document.execCommand('copy'))
            })}
          >
            Copy
          </ContextMenu.Item>
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              const text = window.getSelection()?.toString() ?? ''
              if (text) {
                navigator.clipboard
                  .writeText(text)
                  // biome-ignore lint/suspicious/noExplicitAny: deleteFragment not typed on useEditorRef return
                  .then(() => (editor as any).deleteFragment())
                  .catch(() => document.execCommand('cut'))
              }
            })}
          >
            Cut
          </ContextMenu.Item>
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              navigator.clipboard
                .readText()
                .then((text) => {
                  // biome-ignore lint/suspicious/noExplicitAny: insertText not typed on useEditorRef return
                  if (text) (editor as any).insertText(text)
                })
                .catch(() => document.execCommand('paste'))
            })}
          >
            Paste
          </ContextMenu.Item>

          <ContextMenu.Separator className={separatorCls} />

          {/* ── Formatting ── */}
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              editor.tf.toggleMark('bold')
            })}
          >
            Bold
            <Shortcut>Ctrl+B</Shortcut>
          </ContextMenu.Item>
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              editor.tf.toggleMark('italic')
            })}
          >
            Italic
            <Shortcut>Ctrl+I</Shortcut>
          </ContextMenu.Item>
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              editor.tf.toggleMark('underline')
            })}
          >
            Underline
            <Shortcut>Ctrl+U</Shortcut>
          </ContextMenu.Item>
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              editor.tf.toggleMark('strikethrough')
            })}
          >
            Strikethrough
          </ContextMenu.Item>
          <ContextMenu.Item
            className={itemCls}
            onSelect={withRaf(() => {
              editor.tf.toggleMark('code')
            })}
          >
            Code
          </ContextMenu.Item>

          <ContextMenu.Separator className={separatorCls} />

          {/* ── Turn into submenu ── */}
          <ContextMenu.Sub>
            <ContextMenu.SubTrigger
              className={`${itemCls} data-[state=open]:bg-[var(--surface-elevated)] data-[state=open]:text-[var(--axon-secondary)]`}
            >
              Turn into
              <ChevronRight className="ml-auto size-3 opacity-60" />
            </ContextMenu.SubTrigger>

            <ContextMenu.Portal>
              <ContextMenu.SubContent className={contentCls} sideOffset={4}>
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    editor.tf.toggleBlock('p')
                  })}
                >
                  Paragraph
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    editor.tf.toggleBlock('h1')
                  })}
                >
                  Heading 1
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    editor.tf.toggleBlock('h2')
                  })}
                >
                  Heading 2
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    editor.tf.toggleBlock('h3')
                  })}
                >
                  Heading 3
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    editor.tf.toggleBlock('blockquote')
                  })}
                >
                  Blockquote
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    editor.tf.toggleBlock('code_block')
                  })}
                >
                  Code Block
                </ContextMenu.Item>
                <ContextMenu.Separator className={separatorCls} />
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    // biome-ignore lint/suspicious/noExplicitAny: toggleList not typed on base editor
                    ;(editor as any).tf.toggleList({ listStyleType: 'disc' })
                  })}
                >
                  Bullet List
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={itemCls}
                  onSelect={withRaf(() => {
                    // biome-ignore lint/suspicious/noExplicitAny: toggleList not typed on base editor
                    ;(editor as any).tf.toggleList({ listStyleType: 'decimal' })
                  })}
                >
                  Numbered List
                </ContextMenu.Item>
              </ContextMenu.SubContent>
            </ContextMenu.Portal>
          </ContextMenu.Sub>
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}
