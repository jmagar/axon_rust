'use client'

import { CodeBlockPlugin, CodeLinePlugin } from '@platejs/code-block/react'
import { LinkPlugin } from '@platejs/link/react'
import { ListPlugin } from '@platejs/list/react'
import { ImagePlugin } from '@platejs/media/react'
import {
  TableCellHeaderPlugin,
  TableCellPlugin,
  TablePlugin,
  TableRowPlugin,
} from '@platejs/table/react'
import { createPlatePlugin } from 'platejs/react'

import { CodeBlockElement, CodeLineElement } from '@/components/ui/code-block-node'
import { ImageElement } from '@/components/ui/image-node'
import { LinkElement } from '@/components/ui/link-node'
import { ListElement, ListItemContentElement, ListItemElement } from '@/components/ui/list-node'
import {
  TableCellElement,
  TableCellHeaderElement,
  TableElement,
  TableRowElement,
} from '@/components/ui/table-node'

// Minimal plugins for markdown-deserialized list node types (list/li/lic).
// @platejs/list uses indent-based approach with different node keys, so we
// register plain element plugins matching the MDAST_TO_PLATE mapping instead.
const ListElementPlugin = createPlatePlugin({
  key: 'list',
  node: { isElement: true, component: ListElement },
})

const ListItemPlugin = createPlatePlugin({
  key: 'li',
  node: { isElement: true, component: ListItemElement },
})

const ListItemContentPlugin = createPlatePlugin({
  key: 'lic',
  node: { isElement: true, component: ListItemContentElement },
})

export const ExtendedNodesKit = [
  // Code blocks (fenced ```)
  CodeBlockPlugin.withComponent(CodeBlockElement),
  CodeLinePlugin.withComponent(CodeLineElement),

  // Lists (indent-based, for toolbar-driven list creation + markdown roundtrip)
  ListPlugin,

  // Links
  LinkPlugin.configure({
    options: {
      isUrl: (url: string) => {
        try {
          new URL(url)
          return true
        } catch {
          return false
        }
      },
    },
  }).withComponent(LinkElement),

  // Lists (ul/ol/li from remarkGfm markdown deserialization)
  ListElementPlugin,
  ListItemPlugin,
  ListItemContentPlugin,

  // Tables
  TablePlugin.withComponent(TableElement),
  TableRowPlugin.withComponent(TableRowElement),
  TableCellPlugin.withComponent(TableCellElement),
  TableCellHeaderPlugin.withComponent(TableCellHeaderElement),

  // Images
  ImagePlugin.withComponent(ImageElement),
]
