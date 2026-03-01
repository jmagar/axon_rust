'use client'

import { PlateElement, type PlateElementProps } from 'platejs/react'

export function ListElement(props: PlateElementProps) {
  const ordered = (props.element as unknown as { ordered?: boolean }).ordered
  const Tag = ordered ? 'ol' : 'ul'
  return (
    <PlateElement
      {...props}
      as={Tag}
      className={`my-1.5 space-y-0.5 pl-4 text-[var(--text-secondary)] ${ordered ? 'list-decimal' : 'list-disc'}`}
    >
      {props.children}
    </PlateElement>
  )
}

export function ListItemElement(props: PlateElementProps) {
  return (
    <PlateElement
      {...props}
      as="li"
      className="text-[length:var(--text-md)] leading-[var(--leading-copy)]"
    >
      {props.children}
    </PlateElement>
  )
}

export function ListItemContentElement(props: PlateElementProps) {
  return (
    <PlateElement {...props} as="div">
      {props.children}
    </PlateElement>
  )
}
