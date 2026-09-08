# Full-stack, tooling, and documentation crawl audit

Date: 2026-09-06

## Completion contract

A checkbox may be checked only after its official documentation scope has:

1. completed a bare Axon site crawl invoked as `axon <URL> --scope site` with no other crawl parameters and no terminal crawl error;
2. produced a committed source generation with nonzero indexed document and chunk counts;
3. passed an Axon retrieval probe scoped to that source; and
4. passed a grounded Axon synthesis probe whose citations resolve to that source.

The proof ledger records the crawl job, source/generation, indexed counts, retrieval probe, and synthesis probe. Shared official documentation scopes may satisfy multiple inventory entries.

## Repository stacks

### Axon

- [ ] Axon — [official repository](https://github.com/dinglebear-ai/axon)
- [ ] Rust and Cargo — [The Rust Programming Language](https://doc.rust-lang.org/)
- [ ] Tokio asynchronous runtime — [Tokio documentation](https://tokio.rs/)
- [ ] Axum HTTP framework — [Axum documentation](https://docs.rs/axum/)
- [ ] RMCP / Model Context Protocol — [RMCP](https://docs.rs/rmcp/) and [MCP specification](https://modelcontextprotocol.io/)
- [ ] SQLite and FTS5 — [SQLite documentation](https://sqlite.org/docs.html)
- [ ] Qdrant vector database — [Qdrant documentation](https://qdrant.tech/documentation/)
- [ ] Hugging Face Text Embeddings Inference — [TEI documentation](https://huggingface.co/docs/text-embeddings-inference/)
- [ ] Qwen3 Embedding — [Qwen3 Embedding model documentation](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B)
- [ ] Docker and Docker Compose — [Docker documentation](https://docs.docker.com/)
- [ ] Chrome DevTools Protocol — [CDP documentation](https://chromedevtools.github.io/devtools-protocol/)
- [ ] Reqwest and Rustls — [Reqwest](https://docs.rs/reqwest/) and [Rustls](https://docs.rs/rustls/)
- [ ] OpenAPI — [OpenAPI specification](https://spec.openapis.org/oas/latest.html)
- [ ] OAuth 2.0 and OpenID Connect — [OAuth](https://oauth.net/2/) and [OpenID Connect](https://openid.net/developers/how-connect-works/)
- [ ] React — [React documentation](https://react.dev/)
- [ ] Next.js — [Next.js documentation](https://nextjs.org/docs)
- [ ] TypeScript — [TypeScript documentation](https://www.typescriptlang.org/docs/)
- [ ] Tailwind CSS — [Tailwind CSS documentation](https://tailwindcss.com/docs)
- [ ] Vite and Vitest — [Vite](https://vite.dev/guide/) and [Vitest](https://vitest.dev/guide/)
- [ ] Tauri — [Tauri documentation](https://v2.tauri.app/)
- [ ] Android, Kotlin, and Gradle — [Android Developers](https://developer.android.com/docs), [Kotlin](https://kotlinlang.org/docs/), and [Gradle](https://docs.gradle.org/current/userguide/userguide.html)
- [ ] Chrome Extensions — [Chrome Extensions documentation](https://developer.chrome.com/docs/extensions/)

### Cortex

- [ ] Cortex — [official repository](https://github.com/dinglebear-ai/cortex)
- [ ] Rust, Cargo, Tokio, Axum, RMCP, Reqwest, and Rustls — shared official scopes above
- [ ] SQLite, FTS5, and rusqlite — [SQLite](https://sqlite.org/docs.html) and [rusqlite](https://docs.rs/rusqlite/)
- [ ] Syslog RFC 3164 and RFC 5424 — [IETF RFC 3164](https://datatracker.ietf.org/doc/html/rfc3164) and [RFC 5424](https://datatracker.ietf.org/doc/html/rfc5424)
- [ ] OpenTelemetry and OTLP — [OpenTelemetry documentation](https://opentelemetry.io/docs/)
- [ ] Docker Engine API and Bollard — [Docker Engine API](https://docs.docker.com/reference/api/engine/) and [Bollard](https://docs.rs/bollard/)
- [ ] React, Next.js, TypeScript, and Tailwind CSS — shared official scopes above
- [ ] Playwright — [Playwright documentation](https://playwright.dev/docs/intro)
- [ ] MCP Bundles — [MCPB documentation](https://github.com/anthropics/mcpb)

### Soma

- [ ] Soma — [official repository](https://github.com/dinglebear-ai/soma)
- [ ] Rust, Cargo, Tokio, Axum, RMCP, Reqwest, Rustls, SQLite, and OpenAPI — shared official scopes above
- [ ] PyO3 — [PyO3 user guide](https://pyo3.rs/)
- [ ] Python — [Python documentation](https://docs.python.org/3/)
- [ ] Incus — [Incus documentation](https://linuxcontainers.org/incus/docs/main/)
- [ ] QuickJS and Javy — [QuickJS](https://bellard.org/quickjs/) and [Javy](https://github.com/bytecodealliance/javy)
- [ ] Gotify — [Gotify documentation](https://gotify.net/docs/)
- [ ] UniFi — [UniFi help center](https://help.ui.com/)
- [ ] React, Next.js, TypeScript, Tailwind CSS, Vite, Vitest, and Tauri — shared official scopes above
- [ ] Radix UI — [Radix Primitives documentation](https://www.radix-ui.com/primitives/docs/overview/introduction)
- [ ] Biome — [Biome documentation](https://biomejs.dev/)

### Labby

- [ ] Labby — [official repository](https://github.com/dinglebear-ai/labby)
- [ ] Rust, Cargo, Tokio, Axum, RMCP, Reqwest, Rustls, SQLite, OpenAPI, OAuth, and OIDC — shared official scopes above
- [ ] Javy / QuickJS Code Mode — shared official scopes above
- [ ] JSON Web Tokens — [JWT introduction](https://jwt.io/introduction)
- [ ] React, TypeScript, Tailwind CSS, Vite, Vitest, and Tauri — shared official scopes above
- [ ] MCPB — shared official scope above

### unraid/core

- [ ] Unraid Core — [official Unraid documentation](https://docs.unraid.net/)
- [ ] Elixir — [Elixir documentation](https://hexdocs.pm/elixir/)
- [ ] Erlang/OTP — [Erlang documentation](https://www.erlang.org/doc/)
- [ ] Phoenix — [Phoenix documentation](https://hexdocs.pm/phoenix/)
- [ ] Phoenix LiveView — [LiveView documentation](https://hexdocs.pm/phoenix_live_view/)
- [ ] Ecto — [Ecto documentation](https://hexdocs.pm/ecto/)
- [ ] Absinthe GraphQL — [Absinthe documentation](https://hexdocs.pm/absinthe/)
- [ ] Bandit — [Bandit documentation](https://hexdocs.pm/bandit/)
- [ ] Req — [Req documentation](https://hexdocs.pm/req/)
- [ ] Swoosh — [Swoosh documentation](https://hexdocs.pm/swoosh/)
- [ ] PostHog Elixir — [PostHog documentation](https://posthog.com/docs/)
- [ ] OpenID Connect — shared official scope above
- [ ] GraphQL — [GraphQL documentation](https://graphql.org/learn/)
- [ ] WebDAV — [RFC 4918](https://datatracker.ietf.org/doc/html/rfc4918)
- [ ] Web Push — [W3C Push API](https://www.w3.org/TR/push-api/)
- [ ] Tailwind CSS, esbuild, TypeScript, and Vitest — [Tailwind](https://tailwindcss.com/docs), [esbuild](https://esbuild.github.io/), [TypeScript](https://www.typescriptlang.org/docs/), and [Vitest](https://vitest.dev/guide/)
- [ ] Monaco Editor — [Monaco Editor documentation](https://microsoft.github.io/monaco-editor/)
- [ ] xterm.js — [xterm.js documentation](https://xtermjs.org/docs/)
- [ ] Workbox — [Workbox documentation](https://developer.chrome.com/docs/workbox/)
- [ ] Nix — [Nix documentation](https://nix.dev/)

### unraid/core-plugins

- [ ] Unraid Core Plugins — [official repository](https://github.com/unraid/core-plugins)
- [ ] Elixir, Phoenix, LiveView, Absinthe, Req, Jason, and TypedStruct — [HexDocs](https://hexdocs.pm/)

### dinglebear-ai/my-core-plugs

- [ ] My Core Plugins — [official repository](https://github.com/dinglebear-ai/my-core-plugs)
- [ ] Elixir, Phoenix, LiveView, Absinthe, Req, Jason, and TypedStruct — [HexDocs](https://hexdocs.pm/)

## Installed toolchains and CLIs

Inventory source: active and installed `mise` entries, `rustup` toolchains/components/targets, and `cargo install --list`. Homebrew, pipx, and global npm reported no separately managed packages.

- [ ] actionlint — [documentation](https://github.com/rhysd/actionlint)
- [ ] age — [documentation](https://age-encryption.org/)
- [ ] cargo-deny — [documentation](https://embarkstudios.github.io/cargo-deny/)
- [ ] Mutagen — [documentation](https://mutagen.io/documentation/)
- [ ] ast-grep — [documentation](https://ast-grep.github.io/)
- [ ] Atuin — [documentation](https://docs.atuin.sh/)
- [ ] bat — [documentation](https://github.com/sharkdp/bat)
- [ ] Biome — [documentation](https://biomejs.dev/)
- [ ] Bitwarden CLI — [documentation](https://bitwarden.com/help/cli/)
- [ ] bottom — [documentation](https://clementtsang.github.io/bottom/)
- [ ] Bun — [documentation](https://bun.sh/docs)
- [ ] cargo-binstall — [documentation](https://github.com/cargo-bins/cargo-binstall)
- [ ] bacon — [documentation](https://dystroy.org/bacon/)
- [ ] cargo-audit — [documentation](https://rustsec.org/)
- [ ] cargo-edit — [documentation](https://github.com/killercup/cargo-edit)
- [ ] cargo-generate — [documentation](https://cargo-generate.github.io/cargo-generate/)
- [ ] cargo-llvm-cov — [documentation](https://github.com/taiki-e/cargo-llvm-cov)
- [ ] cargo-machete — [documentation](https://github.com/bnjbvr/cargo-machete)
- [ ] cargo-nextest — [documentation](https://nexte.st/)
- [ ] Taplo CLI — [documentation](https://taplo.tamasfe.dev/)
- [ ] chezmoi — [documentation](https://www.chezmoi.io/)
- [ ] CMake — [documentation](https://cmake.org/cmake/help/latest/)
- [ ] GitHub Copilot CLI — [documentation](https://docs.github.com/en/copilot/how-tos/set-up/install-copilot-cli)
- [ ] Cosign — [documentation](https://docs.sigstore.dev/cosign/)
- [ ] ctop — [documentation](https://github.com/bcicen/ctop)
- [ ] delta — [documentation](https://dandavison.github.io/delta/)
- [ ] Difftastic — [documentation](https://difftastic.wilfred.me.uk/)
- [ ] Dive — [documentation](https://github.com/wagoodman/dive)
- [ ] Docker CLI — [documentation](https://docs.docker.com/reference/cli/docker/)
- [ ] DuckDB — [documentation](https://duckdb.org/docs/stable/)
- [ ] duf — [documentation](https://github.com/muesli/duf)
- [ ] dust — [documentation](https://github.com/bootandy/dust)
- [ ] Elixir — [documentation](https://hexdocs.pm/elixir/)
- [ ] ElixirLS — [documentation](https://github.com/elixir-lsp/elixir-ls)
- [ ] Erlang/OTP — [documentation](https://www.erlang.org/doc/)
- [ ] eza — [documentation](https://eza.rocks/)
- [ ] fd — [documentation](https://github.com/sharkdp/fd)
- [ ] fx — [documentation](https://fx.wtf/)
- [ ] fzf — [documentation](https://github.com/junegunn/fzf)
- [ ] GitHub CLI — [documentation](https://cli.github.com/manual/)
- [ ] git-cliff — [documentation](https://git-cliff.org/docs/)
- [ ] Git LFS — [documentation](https://git-lfs.com/)
- [ ] Dolt — [documentation](https://docs.dolthub.com/)
- [ ] Antigravity CLI — [documentation](https://antigravity.google/docs/home)
- [ ] Gotify CLI — [documentation](https://gotify.net/docs/)
- [ ] Gitleaks — [documentation](https://gitleaks.io/)
- [ ] GitUI — [documentation](https://github.com/gitui-org/gitui)
- [ ] Glow — [documentation](https://github.com/charmbracelet/glow)
- [ ] Go — [documentation](https://go.dev/doc/)
- [ ] golangci-lint — [documentation](https://golangci-lint.run/docs/)
- [ ] Gradle — [documentation](https://docs.gradle.org/current/userguide/userguide.html)
- [ ] gron — [documentation](https://github.com/tomnomnom/gron)
- [ ] Gum — [documentation](https://github.com/charmbracelet/gum)
- [ ] Hadolint — [documentation](https://github.com/hadolint/hadolint)
- [ ] Hyperfine — [documentation](https://github.com/sharkdp/hyperfine)
- [ ] Eclipse Temurin JDK — [documentation](https://adoptium.net/docs/)
- [ ] jless — [documentation](https://jless.io/)
- [ ] jq — [documentation](https://jqlang.org/manual/)
- [ ] just — [documentation](https://just.systems/man/en/)
- [ ] Kotlin — [documentation](https://kotlinlang.org/docs/)
- [ ] kscript — [documentation](https://github.com/kscripting/kscript)
- [ ] ktlint — [documentation](https://pinterest.github.io/ktlint/latest/)
- [ ] lazydocker — [documentation](https://github.com/jesseduffield/lazydocker)
- [ ] lazygit — [documentation](https://github.com/jesseduffield/lazygit)
- [ ] Lefthook — [documentation](https://lefthook.dev/)
- [ ] lnav — [documentation](https://docs.lnav.org/)
- [ ] Apache Maven — [documentation](https://maven.apache.org/guides/)
- [ ] Microsandbox — [documentation](https://docs.microsandbox.dev/)
- [ ] mprocs — [official legacy documentation](https://github.com/pvolok/dekit/blob/master/README-mprocs.md) (the project was renamed to dekit; the legacy CLI remains available)
- [ ] Node.js — [documentation](https://nodejs.org/docs/latest/api/)
- [ ] Gemini CLI — [documentation](https://github.com/google-gemini/gemini-cli)
- [ ] markdown-link-check — [documentation](https://github.com/tcort/markdown-link-check)
- [ ] mcporter — [documentation](https://github.com/steipete/mcporter)
- [ ] OpenWiki — [documentation](https://github.com/langchain-ai/openwiki)
- [ ] Repomix — [documentation](https://repomix.com/guide/)
- [ ] OpenCode — [documentation](https://opencode.ai/docs/)
- [ ] Apprise — [documentation](https://github.com/caronc/apprise/wiki)
- [ ] pnpm — [documentation](https://pnpm.io/)
- [ ] Python — [documentation](https://docs.python.org/3/)
- [ ] Rebar3 — [documentation](https://rebar3.org/docs/)
- [ ] ripgrep — [documentation](https://github.com/BurntSushi/ripgrep)
- [ ] Ruff — [documentation](https://docs.astral.sh/ruff/)
- [ ] Rust and Cargo — [documentation](https://doc.rust-lang.org/)
- [ ] sd — [documentation](https://github.com/chmln/sd)
- [ ] ShellCheck — [documentation](https://www.shellcheck.net/wiki/)
- [ ] tmux — [documentation](https://github.com/tmux/tmux/wiki)
- [ ] Tokei — [documentation](https://github.com/XAMPPRocky/tokei)
- [ ] ty — [documentation](https://docs.astral.sh/ty/)
- [ ] typos — [documentation](https://github.com/crate-ci/typos)
- [ ] Cloudflare Tunnel — [documentation](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
- [ ] pkgx — [documentation](https://docs.pkgx.sh/)
- [ ] usql — [documentation](https://github.com/xo/usql)
- [ ] uv — [documentation](https://docs.astral.sh/uv/)
- [ ] watchexec — [documentation](https://watchexec.github.io/)
- [ ] Yazi — [documentation](https://yazi-rs.github.io/)
- [ ] yq — [documentation](https://mikefarah.gitbook.io/yq/)
- [ ] Zellij — [documentation](https://zellij.dev/documentation/)
- [ ] zoxide — [documentation](https://github.com/ajeetdsouza/zoxide)
- [ ] cargo-xwin — [documentation](https://github.com/rust-cross/cargo-xwin)
- [ ] devclean — [official repository](https://github.com/jmagar/devclean)
- [ ] Text Embeddings Router 1.9.3 — [TEI documentation](https://huggingface.co/docs/text-embeddings-inference/)
- [ ] Rust toolchains stable, nightly, 1.92.0, 1.93, 1.96, 1.97.0, 1.97.1, 1.98, and 1.98.0 — [Rustup documentation](https://rust-lang.github.io/rustup/)
- [ ] Rust targets aarch64-apple-darwin, aarch64-unknown-linux-gnu, and x86_64-pc-windows-msvc — [Rust platform support](https://doc.rust-lang.org/rustc/platform-support.html)

## Installed third-party applications

- [ ] 1Password — [support documentation](https://support.1password.com/)
- [ ] Amphetamine — [official application page](https://apps.apple.com/app/amphetamine/id937984704)
- [ ] Android Studio — [documentation](https://developer.android.com/studio/intro)
- [ ] Antigravity — [documentation](https://antigravity.google/docs/home)
- [ ] Asana — [help center](https://help.asana.com/)
- [ ] Axon Palette — [official repository](https://github.com/dinglebear-ai/axon)
- [ ] Bionic — [LM Studio documentation](https://lmstudio.ai/docs/) (the installed Element Labs bundle declares `lmstudio.ai` as its homepage)
- [ ] Bitwarden — [documentation](https://bitwarden.com/help/)
- [ ] ChatGPT — [OpenAI help center](https://help.openai.com/en/collections/3742473-chatgpt)
- [ ] Claude — [Claude help center](https://support.anthropic.com/)
- [ ] Claude Code URL Handler — [Claude Code documentation](https://docs.anthropic.com/en/docs/claude-code/overview)
- [ ] Delta — [Zed documentation](https://zed.dev/docs/)
- [ ] Apple Developer — [Apple developer documentation](https://developer.apple.com/documentation/)
- [ ] Dia — [Dia help center](https://help.diabrowser.com/)
- [ ] Discord — [support documentation](https://support.discord.com/)
- [ ] Google Chrome — [Chrome help](https://support.google.com/chrome/)
- [ ] Labby — [official repository](https://github.com/dinglebear-ai/labby)
- [ ] Linear — [documentation](https://linear.app/docs/)
- [ ] LM Studio — [documentation](https://lmstudio.ai/docs/)
- [ ] Notion — [help center](https://www.notion.com/help)
- [ ] Open Design — [official repository](https://github.com/nexu-io/open-design) (matched to installed bundle identifier `io.open-design.desktop`)
- [ ] OrbStack — [documentation](https://docs.orbstack.dev/)
- [ ] Parsec — [documentation](https://support.parsec.app/)
- [ ] Raycast — [documentation](https://developers.raycast.com/)
- [ ] Rippling — [help center](https://help.rippling.com/)
- [ ] Roam — [official documentation entry point](https://roamresearch.com/#/app/help)
- [ ] RustDesk — [documentation](https://rustdesk.com/docs/)
- [ ] Safari — [Safari user guide](https://support.apple.com/guide/safari/welcome/mac)
- [ ] Tailscale — [documentation](https://tailscale.com/kb/)
- [ ] Warp — [documentation](https://docs.warp.dev/)
- [ ] Zed Preview — [documentation](https://zed.dev/docs/)

## Installed macOS applications and utilities

The following Apple-provided applications were discovered under `/System/Applications` and `/System/Applications/Utilities`. Their shared official documentation root is the [macOS User Guide](https://support.apple.com/guide/mac-help/welcome/mac).

- [ ] App Store
- [ ] Apps
- [ ] Automator
- [ ] Books
- [ ] Calculator
- [ ] Calendar
- [ ] Chess
- [ ] Clock
- [ ] Contacts
- [ ] Dictionary
- [ ] FaceTime
- [ ] Find My
- [ ] Font Book
- [ ] Freeform
- [ ] Games
- [ ] Home
- [ ] Image Capture
- [ ] Image Playground
- [ ] iPhone Mirroring
- [ ] Journal
- [ ] Mail
- [ ] Maps
- [ ] Messages
- [ ] Mission Control
- [ ] Music
- [ ] News
- [ ] Notes
- [ ] Passwords
- [ ] Phone
- [ ] Photo Booth
- [ ] Photos
- [ ] Podcasts
- [ ] Preview
- [ ] QuickTime Player
- [ ] Reminders
- [ ] Shortcuts
- [ ] Siri
- [ ] Stickies
- [ ] Stocks
- [ ] System Settings
- [ ] TextEdit
- [ ] Time Machine
- [ ] Tips
- [ ] TV
- [ ] Voice Memos
- [ ] Weather
- [ ] Activity Monitor
- [ ] AirPort Utility
- [ ] Audio MIDI Setup
- [ ] Bluetooth File Exchange
- [ ] Boot Camp Assistant
- [ ] ColorSync Utility
- [ ] Console
- [ ] Digital Color Meter
- [ ] Disk Utility
- [ ] Grapher
- [ ] Magnifier
- [ ] Migration Assistant
- [ ] Print Center
- [ ] Screen Sharing
- [ ] Screenshot
- [ ] Script Editor
- [ ] System Information
- [ ] Terminal
- [ ] VoiceOver Utility

## Documentation crawl proof ledger

No entry is complete until all proof columns are populated and the dependent checkboxes above are checked.

| Official documentation scope | Crawl job | Source / generation | Indexed documents / vectors | Retrieval proof | Synthesis proof | Status |
| --- | --- | --- | --- | --- | --- | --- |
