export type LobeSummary = {
  name: string
  repo: string
  collection: string
  branch: string
  sessions: number
  docs: number
  status: string
}

export type WorkflowSession = {
  id: string
  repo: string
  branch: string
  agent: string
  title: string
  status: string
}

export type WorkflowMessage = {
  id: string
  role: 'user' | 'assistant'
  content: string
  reasoning?: string
  files?: string[]
}

export const LOBES: LobeSummary[] = [
  {
    name: 'axon-rust',
    repo: '/home/jmagar/workspace/axon_rust',
    collection: 'lobe_axon_rust',
    branch: 'feat/lobe-workflow-shells',
    sessions: 48,
    docs: 182,
    status: 'live',
  },
  {
    name: 'spider',
    repo: '/home/jmagar/workspace/spider',
    collection: 'lobe_spider',
    branch: 'main',
    sessions: 31,
    docs: 114,
    status: 'stable',
  },
  {
    name: 'nugs',
    repo: '/home/jmagar/workspace/nugs',
    collection: 'lobe_nugs',
    branch: 'feat/graphrag',
    sessions: 22,
    docs: 76,
    status: 'draft',
  },
]

export const LOBE_METRICS = [
  ['CI / CD', '3 workflows green · 1 queued deploy'],
  ['PRs / Reviews', '4 open PRs · 11 review comments'],
  ['Qdrant', '18,221 vectors · 182 docs · 1 collection'],
  ['Agents', 'Claude · Codex · Gemini · Copilot'],
  ['Issues / Todos', '9 open issues · 14 todos'],
  ['Logs / Jobs', '7 services tracked · 3 jobs active'],
] as const

export const LOBE_ROADMAP = [
  'Turn repo, sessions, and crawled docs into one navigable project memory surface.',
  'Let every artifact open in Plate, terminal, logs, or workflow without losing project context.',
  'Treat MCP, skills, agent config, and docs suggestions as living project infrastructure.',
]

export const LOBE_TODOS = [
  'Ingest all agent sessions from the repo path into the lobe collection.',
  'Group Cortex docs by stack and open any page directly in the editor.',
  'Surface PR comments, issues, logs, and MCP config without overwhelming the dashboard.',
]

export const LOBE_INDEXING = [
  ['Repo source', '14,298 files analyzed', 'completed'],
  ['Claude / Codex / Gemini sessions', '48 sessions queued into Qdrant', 'completed'],
  ['PR reviews + issues', '29 artifacts synced from GitHub', 'pending'],
  ['Cortex docs', 'Next.js, Rust, PlateJS, ACP linked', 'pending'],
] as const

export const REPO_TREE = [
  'apps/web/app/reboot/lobe/page.tsx',
  'apps/web/app/reboot/workflow/page.tsx',
  'apps/web/components/reboot/lobe-shell.tsx',
  'apps/web/components/reboot/workflow-shell.tsx',
  'apps/web/components/reboot/data.ts',
  'apps/web/components/pulse/sidebar/pulse-sidebar.tsx',
] as const

export const DOC_LIBRARY = [
  ['Next.js', 'App Router data, caching, route handlers, and streaming UX.'],
  ['Rust', 'Tokio patterns, trait ergonomics, and workspace composition.'],
  ['PlateJS', 'Editor plugins, markdown transforms, and slash commands.'],
  ['ACP', 'Agent transport, session restore, tools, and browser clients.'],
] as const

export const CONFIG_LIBRARY = [
  'AGENTS.md',
  'CLAUDE.md',
  'GEMINI.md',
  '.mcp.json',
  'skills/',
  'agents/',
  'commands/',
] as const

export const OPS_SURFACES = [
  ['Terminal', 'Drop into xterm.js with the project cwd already loaded.'],
  ['Logs', 'Tail workers, web, qdrant, redis, and job lanes without leaving the lobe.'],
  ['Jobs', 'See crawl, extract, embed, and ingest pipelines moving in real time.'],
  ['MCP', 'Edit MCP endpoints, auth, and commands in one project-scoped control surface.'],
  ['Skills', 'Pin repo-specific skills, commands, and agent playbooks.'],
] as const

export const WORKFLOW_SESSIONS: WorkflowSession[] = [
  {
    id: 's1',
    repo: 'axon_rust',
    branch: 'feat/lobe-workflow-shells',
    agent: 'Codex',
    title: 'Split reboot into Lobe and Workflow shells',
    status: 'active',
  },
  {
    id: 's2',
    repo: 'spider',
    branch: 'main',
    agent: 'Claude',
    title: 'Crawler render-mode regression review',
    status: 'idle',
  },
  {
    id: 's3',
    repo: 'nugs',
    branch: 'feat/graphrag',
    agent: 'Gemini',
    title: 'Docs ingestion and graph search planning',
    status: 'review',
  },
  {
    id: 's4',
    repo: 'axon_rust',
    branch: 'main',
    agent: 'Copilot',
    title: 'CI remediation and branch cleanup',
    status: 'queued',
  },
]

export const WORKFLOW_CONVERSATIONS: Record<string, WorkflowMessage[]> = {
  s1: [
    {
      id: 'm1',
      role: 'user',
      content:
        'Seed a new Axon workflow shell from the omnibox, keep the neural background, and make the editor appear when a file is mentioned.',
    },
    {
      id: 'm2',
      role: 'assistant',
      content:
        'I split the problem into two surfaces: the repo-scoped Lobe and the global Workflow shell. The Workflow shell should stay session-centric and open the editor only when a file or artifact becomes active.',
      reasoning:
        'The previous reboot route overloaded one screen with both project-home and active-work concepts. Separating them makes the dashboard calmer and lets the workflow shell optimize for session velocity, pane choreography, and direct artifact handoff.',
      files: [
        'apps/web/components/reboot/workflow-shell.tsx',
        'apps/web/components/reboot/lobe-shell.tsx',
      ],
    },
    {
      id: 'm3',
      role: 'assistant',
      content:
        'I also mocked the research ignition path: crawl docs, pull session memory, then surface a PRD in the editor. Use `Ctrl+Shift+E` or `Cmd+Shift+E` to force-open the editor pane from the keyboard.',
      files: ['docs/plans/lobe-workflow-reboot.md'],
    },
  ],
  s2: [
    {
      id: 'm4',
      role: 'user',
      content:
        'Audit the HTTP to Chrome fallback in render-mode auto-switch and document the failure path.',
    },
    {
      id: 'm5',
      role: 'assistant',
      content:
        'The fallback remains correct. Thin-page ratios above 60% trigger Chrome, but only if a Chrome endpoint is reachable. If Chrome is unavailable, the HTTP result is retained and surfaced to the UI as the final crawl output.',
      files: ['crates/crawl/engine.rs', 'crates/core/content.rs'],
    },
  ],
  s3: [
    {
      id: 'm6',
      role: 'assistant',
      content:
        'Gemini clustered the current docs footprint into Rust, PlateJS, ACP, and Qdrant. The next step is to let the lobe open those docs directly in Plate.',
      files: ['docs/plans/docs-surface.md'],
    },
  ],
  s4: [
    {
      id: 'm7',
      role: 'assistant',
      content:
        'Copilot found one flaky action and a stale cache key in CI. The workflow here is pending review.',
      files: ['.github/workflows/web.yml'],
    },
  ],
}

export const EDITOR_CONTENT: Record<string, string> = {
  'apps/web/components/reboot/workflow-shell.tsx': `# Workflow Shell\n\n## Purpose\nBuild a global work surface that keeps sessions small, chat central, and the editor contextual.\n\n## Requirements\n- Left pane lists active sessions across repos.\n- Center pane owns conversation and omnibox.\n- Right pane opens only when files or artifacts become active.\n- Bottom drawer exposes terminal without stealing the primary layout.\n`,
  'apps/web/components/reboot/lobe-shell.tsx': `# Lobe Shell\n\n## Purpose\nProject home base for repo identity, memory, docs, jobs, logs, config, and launch points into the active workflow.\n\n## Surfaces\n- Dashboard\n- Knowledge\n- Sessions\n- Ops\n`,
  'docs/plans/lobe-workflow-reboot.md': `# Axon Lobe + Workflow Reboot\n\n1. Start from a lobe-scoped omnibox.\n2. Seed research through crawl, RAG, and session ingestion.\n3. Launch into the workflow shell when execution becomes active.\n4. Keep editor and terminal contextual, not always on.\n`,
  'crates/crawl/engine.rs': `// engine.rs\n// Auto-switch uses HTTP first, then retries with Chrome when the crawl is too thin.\n`,
  '.github/workflows/web.yml': `name: web\n\non:\n  pull_request:\n  push:\n    branches: [main]\n`,
}

export const TERMINAL_LINES = [
  '$ cargo test -p axon --lib',
  'running 214 tests',
  '214 passed · 0 failed',
  '$ pnpm exec tsc --noEmit',
  'done in 3.4s',
] as const

export const LOG_LINES = [
  '[axon-web] ws connected · pulse ready',
  '[axon] crawl worker heartbeat ok',
  '[qdrant] lobe_axon_rust · 18,221 vectors',
  '[github] 11 review comments still indexed',
] as const
