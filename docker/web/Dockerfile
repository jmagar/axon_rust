# syntax=docker/dockerfile:1
FROM node:24-slim
WORKDIR /app
# corepack ships with Node 24 — use it to install pnpm without a separate npm call
RUN corepack enable && corepack prepare pnpm@latest --activate
# Copy dep manifests only — source is bind-mounted at runtime for hot reload
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
# next dev binds to all interfaces when HOST=0.0.0.0 (set in docker-compose env)
CMD ["pnpm", "run", "dev"]
