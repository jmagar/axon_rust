'use client'

import { PlateElement, type PlateElementProps } from 'platejs/react'

export function CodeBlockElement(props: PlateElementProps) {
  const lang = (props.element as unknown as { lang?: string }).lang
  return (
    <PlateElement
      {...props}
      as="div"
      className="my-2 overflow-hidden rounded-lg border border-[rgba(175,215,255,0.14)] bg-[rgba(5,10,22,0.65)]"
    >
      {lang && (
        <div className="border-b border-[rgba(175,215,255,0.1)] px-3 py-1 font-mono text-[0.68rem] tracking-widest text-[var(--text-dim)] uppercase">
          {lang}
        </div>
      )}
      <pre className="overflow-x-auto p-3 font-mono text-[0.8rem] leading-[1.6] text-[var(--text-secondary)]">
        <code>{props.children}</code>
      </pre>
    </PlateElement>
  )
}

export function CodeLineElement(props: PlateElementProps) {
  return (
    <PlateElement {...props} as="div">
      {props.children}
    </PlateElement>
  )
}
