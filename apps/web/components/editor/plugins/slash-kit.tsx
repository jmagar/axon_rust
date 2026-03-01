'use client'

import { withTriggerCombobox } from '@platejs/combobox'
import { createSlatePlugin, createTSlatePlugin, KEYS } from 'platejs'
import { toTPlatePlugin } from 'platejs/react'

import { SlashInputElement } from '@/components/ui/slash-node'

/** Inline void element inserted while typing `/` — hosts the combobox UI. */
const BaseSlashInputPlugin = createSlatePlugin({
  key: KEYS.slashInput,
  node: {
    isElement: true,
    isInline: true,
    isVoid: true,
  },
})

/** Triggers the slash-command combobox when the user types `/`. */
const BaseSlashPlugin = createTSlatePlugin({
  key: KEYS.slashCommand,
  options: {
    createComboboxInput: (trigger: string) => ({
      children: [{ text: '' }],
      trigger,
      type: KEYS.slashInput,
    }),
    trigger: '/',
    triggerPreviousCharPattern: /^\s?$/,
  },
  plugins: [BaseSlashInputPlugin],
}).overrideEditor(withTriggerCombobox)

export const SlashPlugin = toTPlatePlugin(BaseSlashPlugin)
export const SlashInputPlugin = toTPlatePlugin(BaseSlashInputPlugin)

export const SlashKit = [SlashPlugin, SlashInputPlugin.withComponent(SlashInputElement)]
