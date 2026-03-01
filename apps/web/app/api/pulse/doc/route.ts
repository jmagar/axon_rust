import { NextResponse } from 'next/server'
import { listPulseDocs, loadPulseDoc } from '@/lib/pulse/storage'

export async function GET(request: Request) {
  try {
    const url = new URL(request.url)
    const filename = url.searchParams.get('filename')

    if (filename) {
      const doc = await loadPulseDoc(filename)
      if (!doc) return NextResponse.json({ error: 'Not found' }, { status: 404 })
      return NextResponse.json(doc)
    }

    const docs = await listPulseDocs()
    return NextResponse.json({ docs })
  } catch (err) {
    return NextResponse.json(
      {
        error: `Failed to load pulse docs: ${err instanceof Error ? err.message : 'unknown error'}`,
      },
      { status: 500 },
    )
  }
}
