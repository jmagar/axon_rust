import { Pool } from 'pg'

const DEFAULT_AXON_PG_URL = 'postgresql://axon:postgres@axon-postgres:5432/axon'

type GlobalWithPgPool = typeof globalThis & {
  __axonJobsPgPool?: Pool
}

const globalWithPgPool = globalThis as GlobalWithPgPool

function createPool(): Pool {
  return new Pool({
    connectionString: process.env.AXON_PG_URL ?? DEFAULT_AXON_PG_URL,
  })
}

export function getJobsPgPool(): Pool {
  if (!globalWithPgPool.__axonJobsPgPool) {
    globalWithPgPool.__axonJobsPgPool = createPool()
  }
  return globalWithPgPool.__axonJobsPgPool
}
