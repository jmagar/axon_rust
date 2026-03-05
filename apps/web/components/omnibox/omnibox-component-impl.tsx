'use client'

import {
  AttachedContextPanel,
  FileSuggestionsPanel,
  ModeAppliedFeedback,
  ModeDropdown,
  OptionsPopover,
} from './omnibox-dropdowns'
import { useOmniboxState } from './omnibox-hooks'
import { OmniboxInputBar } from './omnibox-input-bar'

export function Omnibox() {
  const state = useOmniboxState()

  return (
    <div ref={state.omniboxRef} className="space-y-2">
      <div className="relative">
        <OmniboxInputBar
          input={state.input}
          isProcessing={state.isProcessing}
          statusText={state.statusText}
          statusType={state.statusType}
          completionStatus={state.completionStatus}
          mode={state.mode}
          selectedModeDef={state.selectedModeDef}
          willRunAsCommand={state.willRunAsCommand}
          showModeSelector={state.showModeSelector}
          hasOptions={state.hasOptions}
          activeOptionCount={state.activeOptionCount}
          optionsOpen={state.optionsOpen}
          toolsOpen={state.toolsOpen}
          effectiveDropdownOpen={state.effectiveDropdownOpen}
          modeAppliedLabel={state.modeAppliedLabel}
          placeholderIdx={state.placeholderIdx}
          placeholderVisible={state.placeholderVisible}
          isFocused={state.isFocused}
          mentionTipSeen={state.mentionTipSeen}
          contextUtilizationPercent={state.contextUtilizationPercent}
          workspaceMode={state.workspaceMode}
          workspaceContext={state.workspaceContext}
          workspaceResumeSessionId={state.workspaceResumeSessionId}
          pulseAgent={state.pulseAgent}
          pulseModel={state.pulseModel}
          pulsePermissionLevel={state.pulsePermissionLevel}
          currentMode={state.currentMode}
          isProcessingWithCurrentMode={state.isProcessingWithCurrentMode}
          inputRef={state.inputRef}
          toolsRef={state.toolsRef}
          setInput={state.setInput}
          setDropdownOpen={state.setDropdownOpen}
          setOptionsOpen={state.setOptionsOpen}
          setToolsOpen={state.setToolsOpen}
          setIsFocused={state.setIsFocused}
          setMentionTipSeen={state.setMentionTipSeen}
          setPulseAgent={state.setPulseAgent}
          setPulseModel={state.setPulseModel}
          setPulsePermissionLevel={state.setPulsePermissionLevel}
          execute={state.execute}
          cancel={state.cancel}
          handleKeyDown={state.handleKeyDown}
        />

        <ModeDropdown
          effectiveDropdownOpen={state.effectiveDropdownOpen}
          mentionKind={state.mentionKind}
          activeMentionQuery={state.activeMentionToken?.query}
          groupedModes={state.groupedModes}
          mode={state.mode}
          selectMode={state.selectMode}
        />

        <OptionsPopover
          hasOptions={state.hasOptions}
          optionsOpen={state.optionsOpen}
          mode={state.mode}
          optionValues={state.optionValues}
          onOptionValuesChange={state.setOptionValues}
        />
      </div>

      <FileSuggestionsPanel
        fileSuggestions={state.fileSuggestions}
        mentionKind={state.mentionKind}
        mentionSelectionIndex={state.mentionSelectionIndex}
        omniboxPhase={state.omniboxPhase}
        setMentionSelectionIndex={state.setMentionSelectionIndex}
        applyFileMentionCandidate={state.applyFileMentionCandidate}
      />

      <AttachedContextPanel
        fileContextMentions={state.fileContextMentions}
        onRemoveMention={state.removeFileContextMention}
      />

      <ModeAppliedFeedback modeAppliedLabel={state.modeAppliedLabel} />
    </div>
  )
}
