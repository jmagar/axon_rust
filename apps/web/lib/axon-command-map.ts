/**
 * Axon CLI command/option map for frontend planning and progressive implementation.
 *
 * Source of truth:
 * - crates/core/config/cli.rs (clap commands + options)
 * - crates/core/config/parse.rs (argument mapping)
 * - crates/web/execute.rs (current web executor allow-lists / passthrough)
 */

export type AxonCommandCategory = 'content' | 'rag' | 'ingest' | 'ops' | 'service' | 'workspace'
export type AxonInputKind = 'none' | 'url' | 'urls' | 'text' | 'repo' | 'target' | 'input'
export type AxonRenderIntent =
  | 'markdown-document'
  | 'manifest-browser'
  | 'table'
  | 'cards'
  | 'report'
  | 'job-lifecycle'
  | 'status-summary'
  | 'raw-fallback'
  | 'workspace'

export interface AxonOptionSpec {
  key: string
  flag: string
  value: 'bool' | 'number' | 'string' | 'enum' | 'list'
  scope: 'global' | 'command'
  notes?: string
}

export interface AxonCommandSpec {
  id: string
  category: AxonCommandCategory
  input: AxonInputKind
  asyncByDefault: boolean
  supportsJobs: boolean
  commandOptions: string[]
  renderIntent: AxonRenderIntent
}

export const AXON_JOB_SUBCOMMANDS = [
  'status <job_id>',
  'cancel <job_id>',
  'errors <job_id>',
  'list',
  'cleanup',
  'clear',
  'worker',
  'recover',
] as const

export const AXON_COMMAND_SPECS: ReadonlyArray<AxonCommandSpec> = [
  {
    id: 'scrape',
    category: 'content',
    input: 'urls',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'markdown-document',
  },
  {
    id: 'crawl',
    category: 'content',
    input: 'urls',
    asyncByDefault: true,
    supportsJobs: true,
    commandOptions: [],
    renderIntent: 'manifest-browser',
  },
  {
    id: 'map',
    category: 'content',
    input: 'url',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'table',
  },
  {
    id: 'extract',
    category: 'content',
    input: 'urls',
    asyncByDefault: true,
    supportsJobs: true,
    commandOptions: [],
    renderIntent: 'cards',
  },
  {
    id: 'screenshot',
    category: 'content',
    input: 'urls',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'cards',
  },

  {
    id: 'search',
    category: 'rag',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'raw-fallback',
  },
  {
    id: 'research',
    category: 'rag',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'report',
  },
  {
    id: 'query',
    category: 'rag',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'cards',
  },
  {
    id: 'retrieve',
    category: 'rag',
    input: 'url',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'table',
  },
  {
    id: 'ask',
    category: 'rag',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: ['diagnostics'],
    renderIntent: 'report',
  },
  {
    id: 'evaluate',
    category: 'rag',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: ['diagnostics'],
    renderIntent: 'report',
  },
  {
    id: 'suggest',
    category: 'rag',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'table',
  },

  {
    id: 'github',
    category: 'ingest',
    input: 'repo',
    asyncByDefault: true,
    supportsJobs: true,
    commandOptions: ['include_source'],
    renderIntent: 'job-lifecycle',
  },
  {
    id: 'reddit',
    category: 'ingest',
    input: 'target',
    asyncByDefault: true,
    supportsJobs: true,
    commandOptions: ['sort', 'time', 'max_posts', 'min_score', 'depth', 'scrape_links'],
    renderIntent: 'job-lifecycle',
  },
  {
    id: 'youtube',
    category: 'ingest',
    input: 'url',
    asyncByDefault: true,
    supportsJobs: true,
    commandOptions: [],
    renderIntent: 'job-lifecycle',
  },
  {
    id: 'sessions',
    category: 'ingest',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: true,
    commandOptions: ['claude', 'codex', 'gemini', 'project'],
    renderIntent: 'job-lifecycle',
  },
  {
    id: 'embed',
    category: 'ops',
    input: 'input',
    asyncByDefault: true,
    supportsJobs: true,
    commandOptions: [],
    renderIntent: 'job-lifecycle',
  },
  {
    id: 'sources',
    category: 'ops',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'table',
  },
  {
    id: 'domains',
    category: 'ops',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'table',
  },
  {
    id: 'stats',
    category: 'ops',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'status-summary',
  },
  {
    id: 'status',
    category: 'ops',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'table',
  },
  {
    id: 'dedupe',
    category: 'ops',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'status-summary',
  },

  {
    id: 'doctor',
    category: 'service',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'report',
  },
  {
    id: 'debug',
    category: 'service',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'report',
  },
  {
    id: 'serve',
    category: 'service',
    input: 'none',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: ['port'],
    renderIntent: 'raw-fallback',
  },
  {
    id: 'pulse',
    category: 'workspace',
    input: 'text',
    asyncByDefault: false,
    supportsJobs: false,
    commandOptions: [],
    renderIntent: 'workspace',
  },
] as const

export const AXON_COMMAND_OPTIONS: ReadonlyArray<AxonOptionSpec> = [
  {
    key: 'diagnostics',
    flag: '--diagnostics',
    value: 'bool',
    scope: 'command',
    notes: 'ask/evaluate',
  },
  {
    key: 'include_source',
    flag: '--include-source',
    value: 'bool',
    scope: 'command',
    notes: 'github',
  },
  {
    key: 'sort',
    flag: '--sort',
    value: 'enum',
    scope: 'command',
    notes: 'reddit: hot|top|new|rising',
  },
  {
    key: 'time',
    flag: '--time',
    value: 'enum',
    scope: 'command',
    notes: 'reddit: hour|day|week|month|year|all',
  },
  { key: 'max_posts', flag: '--max-posts', value: 'number', scope: 'command', notes: 'reddit' },
  { key: 'min_score', flag: '--min-score', value: 'number', scope: 'command', notes: 'reddit' },
  { key: 'depth', flag: '--depth', value: 'number', scope: 'command', notes: 'reddit' },
  { key: 'scrape_links', flag: '--scrape-links', value: 'bool', scope: 'command', notes: 'reddit' },
  { key: 'claude', flag: '--claude', value: 'bool', scope: 'command', notes: 'sessions' },
  { key: 'codex', flag: '--codex', value: 'bool', scope: 'command', notes: 'sessions' },
  { key: 'gemini', flag: '--gemini', value: 'bool', scope: 'command', notes: 'sessions' },
  { key: 'project', flag: '--project', value: 'string', scope: 'command', notes: 'sessions' },
  { key: 'port', flag: '--port', value: 'number', scope: 'command', notes: 'serve' },
] as const

export function getCommandSpec(id: string): AxonCommandSpec | undefined {
  return AXON_COMMAND_SPECS.find((s) => s.id === id)
}

export function isAsyncMode(id: string): boolean {
  return getCommandSpec(id)?.asyncByDefault ?? false
}

export function isNoInputMode(id: string): boolean {
  const spec = getCommandSpec(id)
  return spec?.input === 'none'
}

export function getCommandsByCategory(category: AxonCommandCategory): AxonCommandSpec[] {
  return AXON_COMMAND_SPECS.filter((s) => s.category === category)
}

// Global options, executor constants, and mode picker are in axon-options.ts
export {
  AXON_GLOBAL_OPTIONS,
  MODE_PICKER_COMMANDS,
  WEB_EXECUTOR_ALLOWED_MODES,
  WEB_EXECUTOR_ASYNC_MODES,
  WEB_EXECUTOR_FILE_AUTOLOAD_MODES,
  WEB_EXECUTOR_FLAG_PASSTHROUGH,
} from './axon-options'
