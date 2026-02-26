'use client'

import type { ChatMessage } from './pulse-workspace'

interface PulseChatPaneProps {
  messages: ChatMessage[]
  isLoading: boolean
}

export function PulseChatPane({ messages, isLoading }: PulseChatPaneProps) {
  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-[rgba(255,135,175,0.08)] px-4 py-2.5">
        <span className="text-[10px] font-bold uppercase tracking-[0.15em] text-[var(--axon-text-dim)]">
          Pulse Chat
        </span>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {messages.length === 0 && isLoading ? (
          <div className="flex h-full items-center justify-center">
            <div className="flex items-center gap-2 text-xs text-[var(--axon-text-dim)]">
              <span className="inline-block size-1.5 animate-pulse rounded-full bg-[var(--axon-accent-pink)]" />
              Thinking...
            </div>
          </div>
        ) : messages.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <p className="text-center text-xs text-[var(--axon-text-dim)]">
              Type a prompt in the omnibox to start.
              <br />
              Pulse will search your knowledge base and help edit this document.
            </p>
          </div>
        ) : (
          <div className="space-y-4">
            {messages.map((msg, i) => (
              <div
                key={`msg-${i}-${msg.role}`}
                className={`rounded-lg p-3 text-sm ${
                  msg.role === 'user'
                    ? 'ml-8 bg-[rgba(175,215,255,0.08)] text-[var(--axon-text-primary)]'
                    : 'mr-8 bg-[rgba(255,135,175,0.06)] text-[var(--axon-text-secondary)]'
                }`}
              >
                {msg.content}
              </div>
            ))}
          </div>
        )}
        {isLoading && messages.length > 0 && (
          <div className="mt-3 flex items-center gap-2 text-xs text-[var(--axon-text-dim)]">
            <span className="inline-block size-1.5 animate-pulse rounded-full bg-[var(--axon-accent-pink)]" />
            Thinking...
          </div>
        )}
      </div>
    </div>
  )
}
