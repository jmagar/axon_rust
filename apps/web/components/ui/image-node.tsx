'use client'

import { PlateElement, type PlateElementProps } from 'platejs/react'

export function ImageElement(props: PlateElementProps) {
  const url = (props.element as unknown as { url?: string }).url
  const alt = (props.element as unknown as { url?: string; alt?: string }).alt
  return (
    <PlateElement {...props} as="div" className="my-2">
      {url && (
        // biome-ignore lint/performance/noImgElement: editor images don't need next/image optimization
        <img
          src={url}
          alt={alt ?? ''}
          className="max-w-full rounded-lg border border-[rgba(175,215,255,0.1)]"
        />
      )}
      {props.children}
    </PlateElement>
  )
}
