'use client'

import { BaseCalloutPlugin } from '@platejs/callout'
import { toTPlatePlugin } from 'platejs/react'

import { CalloutElement } from '@/components/ui/callout-node'

export const CalloutKit = [toTPlatePlugin(BaseCalloutPlugin).withComponent(CalloutElement)]
