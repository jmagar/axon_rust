import { describe, expect, it } from 'vitest'
import { listPulseDocs, loadPulseDoc, savePulseDoc } from '@/lib/pulse/storage'

describe('pulse storage', () => {
  it('saves a pulse document to cache', async () => {
    const saved = await savePulseDoc({ title: 'Storage Test', markdown: '# Hello' })
    expect(saved.filename.endsWith('.md')).toBe(true)
    expect(saved.path.includes('.cache/pulse')).toBe(true)
  })

  it('loads a saved pulse document', async () => {
    const saved = await savePulseDoc({ title: 'Load Test', markdown: 'Body', tags: ['x'] })
    const loaded = await loadPulseDoc(saved.filename)
    expect(loaded?.title).toBe('Load Test')
    expect(loaded?.markdown.trim()).toBe('Body')
    expect(loaded?.tags).toContain('x')
  })

  it('returns null when document does not exist', async () => {
    const loaded = await loadPulseDoc('does-not-exist.md')
    expect(loaded).toBeNull()
  })

  it('lists docs sorted by updatedAt', async () => {
    await savePulseDoc({ title: 'List Test A', markdown: 'A' })
    await savePulseDoc({ title: 'List Test B', markdown: 'B' })
    const docs = await listPulseDocs()
    expect(docs.length).toBeGreaterThan(1)
    for (let i = 1; i < docs.length; i += 1) {
      expect(docs[i - 1].updatedAt >= docs[i].updatedAt).toBe(true)
    }
  })

  it('preserves collections metadata', async () => {
    const saved = await savePulseDoc({
      title: 'Collections Test',
      markdown: 'Body',
      collections: ['pulse', 'cortex'],
    })
    const loaded = await loadPulseDoc(saved.filename)
    expect(loaded?.collections).toEqual(['pulse', 'cortex'])
  })
})
