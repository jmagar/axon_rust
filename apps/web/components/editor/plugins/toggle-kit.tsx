'use client'

import { BaseTogglePlugin } from '@platejs/toggle'
import { toTPlatePlugin } from 'platejs/react'

import { ToggleElement } from '@/components/ui/toggle-node'

export const ToggleKit = [toTPlatePlugin(BaseTogglePlugin).withComponent(ToggleElement)]
