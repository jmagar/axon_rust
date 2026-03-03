'use client'

import { Component, type ErrorInfo, type ReactNode } from 'react'

interface Props {
  children: ReactNode
}

interface State {
  error: Error | null
  resetKey: number
}

export class PulseErrorBoundary extends Component<Props, State> {
  state: State = { error: null, resetKey: 0 }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[PulseWorkspace] uncaught error:', error, info.componentStack)
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-full items-center justify-center p-8">
          <div className="max-w-md space-y-3 rounded-xl border border-[rgba(255,135,135,0.3)] bg-[rgba(127,29,29,0.18)] p-6 text-center">
            <h2 className="text-base font-semibold text-rose-200">Something went wrong</h2>
            <p className="text-sm text-[var(--text-dim)]">{this.state.error.message}</p>
            <button
              type="button"
              onClick={() => this.setState((s) => ({ error: null, resetKey: s.resetKey + 1 }))}
              className="rounded border border-[rgba(135,175,255,0.3)] bg-[rgba(135,175,255,0.12)] px-3 py-1.5 text-sm text-[var(--axon-primary)] hover:bg-[rgba(135,175,255,0.2)]"
            >
              Try again
            </button>
          </div>
        </div>
      )
    }
    return <div key={this.state.resetKey}>{this.props.children}</div>
  }
}
