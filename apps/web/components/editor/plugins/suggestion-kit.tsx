'use client'

import { type BaseSuggestionConfig, BaseSuggestionPlugin } from '@platejs/suggestion'
import type { ExtendConfig, Path } from 'platejs'
import { isSlateEditor, isSlateString } from 'platejs'
import { type RenderNodeWrapper, toTPlatePlugin } from 'platejs/react'

import { SuggestionLeaf, SuggestionLineBreak } from '@/components/ui/suggestion-node'

import { discussionPlugin } from './discussion-kit'

export interface SuggestionConfig
  extends ExtendConfig<
    BaseSuggestionConfig,
    {
      activeId: string | null
      hoverId: string | null
      uniquePathMap: Map<string, Path>
    }
  > {}

export const suggestionPlugin = toTPlatePlugin<SuggestionConfig>(
  BaseSuggestionPlugin,
  ({ editor }) => ({
    options: {
      activeId: null,
      currentUserId: editor.getOption(discussionPlugin, 'currentUserId'),
      hoverId: null,
      uniquePathMap: new Map(),
    },
  }),
).configure({
  handlers: {
    // unset active suggestion when clicking outside of suggestion
    onClick: ({ api, event, setOption, type }) => {
      if (!(event.target instanceof HTMLElement)) {
        setOption('activeId', null)
        return
      }

      let leaf = event.target
      let isSet = false

      const isBlockLeaf = leaf.dataset.blockSuggestion === 'true'

      const unsetActiveSuggestion = () => {
        setOption('activeId', null)
        isSet = true
      }

      if (!isSlateString(leaf) && !isBlockLeaf) {
        unsetActiveSuggestion()
      }

      while (leaf.parentElement && !isSlateEditor(leaf.parentElement)) {
        const isBlockSuggestion = leaf.dataset.blockSuggestion === 'true'

        if (leaf.classList.contains(`slate-${type}`) || isBlockSuggestion) {
          const suggestionEntry = api.suggestion!.node({
            isText: !isBlockSuggestion,
          })

          if (!suggestionEntry) {
            unsetActiveSuggestion()

            break
          }

          const id = api.suggestion!.nodeId(suggestionEntry[0])
          setOption('activeId', id ?? null)

          isSet = true

          break
        }

        leaf = leaf.parentElement
      }

      if (!isSet) unsetActiveSuggestion()
    },
  },
  render: {
    belowNodes: SuggestionLineBreak as unknown as RenderNodeWrapper,
    // biome-ignore lint/suspicious/noExplicitAny: Plate.js render config type mismatch
    node: SuggestionLeaf as any,
    // biome-ignore lint/suspicious/noExplicitAny: Plate.js plugin render shape mismatch
  } as any,
})

export const SuggestionKit = [suggestionPlugin]
