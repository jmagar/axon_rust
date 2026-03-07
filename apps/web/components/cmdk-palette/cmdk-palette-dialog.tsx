'use client'

import { Command } from 'cmdk'
import { Search } from 'lucide-react'
import type { RefObject } from 'react'
import {
  MODE_CATEGORY_LABELS,
  MODE_CATEGORY_ORDER,
  MODES,
  type ModeDefinition,
} from '@/lib/ws-protocol'
import { CmdKOutput } from './CmdKOutput'
import type { PaletteDialogState, PalettePhase } from './cmdk-palette-types'

const URL_MODES = new Set(['scrape', 'crawl', 'map', 'extract', 'retrieve'])
const PALETTE_CATEGORIES: ReadonlySet<string> = new Set(['content', 'rag'])

const PALETTE_STYLES = `
  [cmdk-root] { display: flex; flex-direction: column; flex: 1; min-height: 0; }
  [cmdk-input-wrapper] {
    border-bottom: 1px solid var(--border-subtle);
  }
  [cmdk-input] {
    width: 100%; background: transparent; outline: none; border: none;
    color: var(--axon-primary);
    caret-color: var(--axon-primary);
    font-family: var(--font-mono);
    font-size: var(--text-base);
    padding: 18px 20px;
  }
  [cmdk-input]::placeholder { color: var(--text-dim); }
  [cmdk-list] {
    overflow-y: auto; flex: 1; padding: 6px 8px;
    max-height: calc(70vh - 80px);
  }
  [cmdk-group-heading] {
    font-size: var(--text-2xs);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-dim);
    padding: 8px 10px 4px;
    font-family: var(--font-mono);
  }
  [cmdk-item] {
    display: flex; align-items: center; gap: 10px;
    padding: 10px 12px; border-radius: 8px; cursor: pointer;
    font-family: var(--font-mono);
    font-size: var(--text-md);
    color: var(--text-secondary);
    transition: background 100ms, box-shadow 100ms, color 100ms;
  }
  [cmdk-item][data-selected=true] {
    background: rgba(255, 135, 175, 0.08);
    box-shadow: inset 0 0 0 1px rgba(255, 135, 175, 0.2);
    color: var(--axon-primary);
  }
  [cmdk-item]:hover:not([data-selected=true]) {
    background: var(--surface-primary);
  }
  [cmdk-empty] {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 32px 24px;
    font-family: var(--font-sans);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
`

interface SelectPanelProps {
  search: string
  setSearch: (value: string) => void
  handleSelectMode: (mode: ModeDefinition) => void
}

function SelectPanel({ search, setSearch, handleSelectMode }: SelectPanelProps) {
  return (
    <Command>
      <div data-cmdk-input-wrapper="">
        <Command.Input
          placeholder="Search commands..."
          value={search}
          onValueChange={setSearch}
          autoFocus
        />
      </div>
      <Command.List>
        <Command.Empty>
          <Search className="size-4 opacity-40" />
          <span>No commands found</span>
        </Command.Empty>
        {MODE_CATEGORY_ORDER.filter((cat) => PALETTE_CATEGORIES.has(cat)).map((cat) => {
          const items = MODES.filter((m) => m.category === cat)
          if (!items.length) return null
          return (
            <Command.Group key={cat} heading={MODE_CATEGORY_LABELS[cat]}>
              {items.map((mode) => (
                <Command.Item
                  key={mode.id}
                  value={`${mode.label} ${mode.id}`}
                  onSelect={() => handleSelectMode(mode)}
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.8"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    style={{ flexShrink: 0, opacity: 0.65 }}
                  >
                    <path d={mode.icon} />
                  </svg>
                  {mode.label}
                </Command.Item>
              ))}
            </Command.Group>
          )
        })}
      </Command.List>
    </Command>
  )
}

interface InputPanelProps {
  selectedMode: ModeDefinition
  inputValue: string
  setInputValue: (value: string) => void
  inputRef: RefObject<HTMLInputElement | null>
  onBack: () => void
  handleExecute: () => void
}

function InputPanel({
  selectedMode,
  inputValue,
  setInputValue,
  inputRef,
  onBack,
  handleExecute,
}: InputPanelProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column' }}>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          padding: '10px 16px 6px',
          borderBottom: '1px solid var(--border-subtle)',
        }}
      >
        <button
          type="button"
          onClick={onBack}
          style={{
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            color: 'var(--text-muted)',
            fontSize: 'var(--text-md)',
            padding: '0 4px',
            fontFamily: 'var(--font-mono)',
          }}
        >
          ←
        </button>
        <span
          className="ui-chip"
          style={{
            color: 'var(--axon-primary)',
            background: 'var(--surface-primary-active)',
            border: '1px solid var(--border-standard)',
            borderRadius: 5,
            padding: '2px 8px',
          }}
        >
          {selectedMode.label}
        </span>
      </div>
      <input
        ref={inputRef}
        value={inputValue}
        onChange={(e) => setInputValue(e.target.value)}
        placeholder={URL_MODES.has(selectedMode.id) ? 'https://example.com' : 'What is...'}
        onKeyDown={(e) => {
          if (e.key === 'Enter') handleExecute()
        }}
        style={{
          width: '100%',
          background: 'transparent',
          outline: 'none',
          border: 'none',
          color: 'var(--axon-primary)',
          caretColor: 'var(--axon-primary)',
          fontFamily: 'var(--font-mono)',
          fontSize: 'var(--text-base)',
          padding: '18px 20px',
          boxSizing: 'border-box',
        }}
      />
    </div>
  )
}

interface PaletteDialogProps {
  state: PaletteDialogState
  setPhase: (phase: PalettePhase) => void
  setSearch: (value: string) => void
  setInputValue: (value: string) => void
  inputRef: RefObject<HTMLInputElement | null>
  handleSelectMode: (mode: ModeDefinition) => void
  handleExecute: () => void
  closeToIdle: () => void
  minimizeToBackground: () => void
  cancelAndClose: () => void
  handleOpenInEditor: () => void
}

export function PaletteDialog({
  state,
  setPhase,
  setSearch,
  setInputValue,
  inputRef,
  handleSelectMode,
  handleExecute,
  closeToIdle,
  minimizeToBackground,
  cancelAndClose,
  handleOpenInEditor,
}: PaletteDialogProps) {
  // Running: backdrop dismiss minimizes to background (keeps job alive)
  // Done/select/input: backdrop closes
  const onBackdropClick = state.phase === 'running' ? minimizeToBackground : closeToIdle

  return (
    <>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop dismiss is a recognized UX pattern for modals */}
      <div
        className="fixed inset-0 bg-black/40"
        style={{ zIndex: 100 }}
        onClick={onBackdropClick}
      />

      <div
        className="animate-cmdk-in"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        style={{
          position: 'fixed',
          top: '50%',
          left: '50%',
          zIndex: 101,
          width: 'min(640px, 92vw)',
          maxHeight: '70vh',
          background: 'rgba(10, 18, 35, 0.97)',
          border: '1px solid var(--border-standard)',
          borderRadius: 14,
          boxShadow: 'var(--shadow-xl)',
          backdropFilter: 'blur(24px)',
          WebkitBackdropFilter: 'blur(24px)',
          fontFamily: 'var(--font-mono)',
          overflow: 'hidden',
          display: 'flex',
          flexDirection: 'column',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <style>{PALETTE_STYLES}</style>

        {state.phase === 'select' && (
          <SelectPanel
            search={state.search}
            setSearch={setSearch}
            handleSelectMode={handleSelectMode}
          />
        )}

        {state.phase === 'input' && state.selectedMode && (
          <InputPanel
            selectedMode={state.selectedMode}
            inputValue={state.inputValue}
            setInputValue={setInputValue}
            inputRef={inputRef}
            onBack={() => setPhase('select')}
            handleExecute={handleExecute}
          />
        )}

        {(state.phase === 'running' || state.phase === 'done') && state.selectedMode && (
          <CmdKOutput
            mode={state.selectedMode}
            lines={state.lines}
            jsonCount={state.jsonCount}
            capturedJson={state.capturedJson}
            progress={state.progress}
            exitCode={state.exitCode}
            errorMsg={state.errorMsg}
            elapsedMs={state.elapsedMs}
            jobId={state.jobId}
            onDismiss={closeToIdle}
            onCancel={cancelAndClose}
            onMinimize={minimizeToBackground}
            onOpenInEditor={handleOpenInEditor}
            phase={state.phase}
          />
        )}
      </div>
    </>
  )
}
