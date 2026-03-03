export interface CompletionStatus {
  type: 'done' | 'error'
  text: string
  exitCode?: number
}
