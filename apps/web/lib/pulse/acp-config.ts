import type { AcpConfigOption } from '@/lib/pulse/types'

function toLower(value: string | undefined): string {
  return (value ?? '').trim().toLowerCase()
}

function looksLikeModelConfig(option: AcpConfigOption): boolean {
  const category = toLower(option.category)
  if (category === 'model') {
    return true
  }
  const id = toLower(option.id)
  if (id.includes('model')) {
    return true
  }
  const name = toLower(option.name)
  return name.includes('model')
}

export function getAcpModelConfigOption(options: AcpConfigOption[]): AcpConfigOption | undefined {
  if (options.length === 0) return undefined
  return options.find(looksLikeModelConfig)
}
