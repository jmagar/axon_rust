# syntax=docker/dockerfile:1
FROM node:24-slim

# Install system deps + AI CLIs as root into system paths
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates openssh-client && rm -rf /var/lib/apt/lists/*

# Claude: installer drops a symlink into $HOME/.local/bin pointing to the real
# binary under $HOME/.local/share/claude/versions/. Move the actual binary
# (dereference the symlink) so that non-root users can execute it without
# needing to traverse /root/.local/ (which is 700).
RUN curl -fsSL https://claude.ai/install.sh | bash && \
    cp "$(readlink -f /root/.local/bin/claude)" /usr/local/bin/claude && \
    chmod 755 /usr/local/bin/claude

# Codex + Gemini CLI
RUN npm install -g @openai/codex@0.105.0 --no-fund --no-audit
RUN npm install -g @google/gemini-cli@latest --no-fund --no-audit

# corepack ships with Node 24 — use it to install pnpm without a separate npm call
RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app

# Pre-create dirs that anonymous volumes will mount over, owned by node
# so the node user can write into them at runtime
RUN mkdir -p /app/.next /app/node_modules && chown -R node:node /app

# Copy dep manifests only — source is bind-mounted at runtime for hot reload
COPY --chown=node:node package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

# Drop to non-root for runtime
USER node

# next dev binds to all interfaces when HOST=0.0.0.0 (set in docker-compose env)
CMD ["pnpm", "run", "dev"]
