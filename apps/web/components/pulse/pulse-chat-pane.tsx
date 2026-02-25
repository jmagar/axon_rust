'use client'

import type { ChatMessage } from './pulse-workspace'

interface PulseChatPaneProps {
  messages: ChatMessage[]
  isLoading: boolean
}

export function PulseChatPane({ messages, isLoading }: PulseChatPaneProps) {
  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-[rgba(175,215,255,0.08)] px-4 py-2.5">
        <span className="text-[10px] font-bold uppercase tracking-[0.15em] text-[#5f87af]">
          Pulse Chat
        </span>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {messages.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <p className="text-center text-xs text-[#5f87af]">
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
                    ? 'ml-8 bg-[rgba(255,135,175,0.08)] text-[#e8f4f8]'
                    : 'mr-8 bg-[rgba(175,215,255,0.06)] text-[#c4daf0]'
                }`}
              >
                {msg.content}
              </div>
            ))}
          </div>
        )}
        {isLoading && (
          <div className="mt-3 flex items-center gap-2 text-xs text-[#5f87af]">
            <span className="inline-block size-1.5 animate-pulse rounded-full bg-[#ff87af]" />
            Thinking...
          </div>
        )}
      </div>
    </div>
  )
}
