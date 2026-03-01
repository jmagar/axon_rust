'use client'

import { BaseTocPlugin } from '@platejs/toc'
import { toTPlatePlugin } from 'platejs/react'

import { TocElement } from '@/components/ui/toc-node'

export const TocKit = [toTPlatePlugin(BaseTocPlugin).withComponent(TocElement)]
