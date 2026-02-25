function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function humanizeKey(key: string): string {
  return key
    .replace(/_/g, ' ')
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/\s+/g, ' ')
    .trim()
}

function formatScalar(value: string | number | boolean | null): string {
  if (value === null) return 'none'
  if (typeof value === 'string') return value
  if (typeof value === 'number') return Number.isFinite(value) ? String(value) : 'not-a-number'
  return value ? 'yes' : 'no'
}

function indentBlock(text: string, depth: number): string {
  const indent = '  '.repeat(depth)
  return text
    .split('\n')
    .map((line) => `${indent}${line}`)
    .join('\n')
}

function formatValue(value: unknown, depth: number): string {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    return formatScalar(value)
  }

  if (Array.isArray(value)) {
    if (value.length === 0) return '(empty list)'
    return value
      .map((item, index) => {
        if (
          item === null ||
          typeof item === 'string' ||
          typeof item === 'number' ||
          typeof item === 'boolean'
        ) {
          return `- ${formatScalar(item)}`
        }
        return `- Item ${index + 1}\n${indentBlock(formatValue(item, depth + 1), 1)}`
      })
      .join('\n')
  }

  if (isRecord(value)) {
    const entries = Object.entries(value).filter(([, v]) => v !== undefined)
    if (entries.length === 0) return '(empty object)'

    return entries
      .map(([key, v]) => {
        const label = humanizeKey(key)
        if (
          v === null ||
          typeof v === 'string' ||
          typeof v === 'number' ||
          typeof v === 'boolean'
        ) {
          return `${label}: ${formatScalar(v)}`
        }
        return `${label}:\n${indentBlock(formatValue(v, depth + 1), 1)}`
      })
      .join('\n')
  }

  return String(value)
}

export function formatStructuredText(value: unknown): string {
  return formatValue(value, 0)
}

export function summarizeStructuredValue(value: unknown): string {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    const text = formatScalar(value)
    return text.length > 96 ? `${text.slice(0, 93)}...` : text
  }

  if (Array.isArray(value)) {
    if (value.length === 0) return '(empty list)'
    const preview = value
      .slice(0, 3)
      .map((item) => {
        if (
          item === null ||
          typeof item === 'string' ||
          typeof item === 'number' ||
          typeof item === 'boolean'
        ) {
          return formatScalar(item)
        }
        if (Array.isArray(item)) return `list(${item.length})`
        return 'object'
      })
      .join(', ')
    return value.length > 3 ? `${preview}, ...` : preview
  }

  if (isRecord(value)) {
    const entries = Object.entries(value)
      .filter(([, v]) => v !== undefined)
      .slice(0, 3)
      .map(([k, v]) => {
        const key = humanizeKey(k)
        if (
          v === null ||
          typeof v === 'string' ||
          typeof v === 'number' ||
          typeof v === 'boolean'
        ) {
          return `${key}: ${formatScalar(v)}`
        }
        if (Array.isArray(v)) return `${key}: list(${v.length})`
        return `${key}: object`
      })
    const base = entries.join(' | ')
    const total = Object.keys(value).length
    return total > 3 ? `${base} | ...` : base
  }

  return String(value)
}
