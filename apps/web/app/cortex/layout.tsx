import type { ReactNode } from 'react'

export default function CortexLayout({ children }: { children: ReactNode }) {
  return <div className="mx-auto max-w-5xl p-6">{children}</div>
}
