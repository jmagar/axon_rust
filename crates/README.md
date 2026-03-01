# crates/
Last Modified: 2026-02-25

Module index for Axon’s Rust crate directories.

## Runtime Modules
- [cli](./cli/README.md): command routing and CLI command handlers.
- [core](./core/README.md): shared config, HTTP/content, logging, and health primitives.
- [crawl](./crawl/README.md): crawl engine and crawl manifest logic.
- [ingest](./ingest/README.md): source adapters (GitHub, Reddit, YouTube, sessions).
- [jobs](./jobs/README.md): async queue workers and job lifecycle management.
- [mcp](./mcp/README.md): MCP server crate for `axon-mcp`.
- [vector](./vector/README.md): embeddings, Qdrant operations, retrieval, and RAG.
- [web](./web/README.md): `axon serve` web runtime and websocket bridge.

## Re-export Shims
These top-level Rust files re-export module roots used by the workspace crate graph:
- `cli.rs`
- `core.rs`
- `crawl.rs`
- `ingest.rs`
- `jobs.rs`
- `vector.rs`
- `web.rs`

## Related Docs
- [Repository README](../README.md)
- [Architecture](../docs/ARCHITECTURE.md)
- [Docs Index](../docs/README.md)
