#!/usr/bin/env node
/**
 * Generates PWA icons (192x192 and 512x512 PNG) using only Node built-ins.
 * Run: node scripts/generate-pwa-icons.mjs
 * Output: public/icons/icon-192.png, public/icons/icon-512.png
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { deflateSync } from 'node:zlib'

const __dirname = dirname(fileURLToPath(import.meta.url))
const OUT_DIR = resolve(__dirname, '../public/icons')

// ── CRC32 table (PNG chunk CRCs) ──────────────────────────────────────────────
const CRC_TABLE = (() => {
  const t = new Uint32Array(256)
  for (let i = 0; i < 256; i++) {
    let c = i
    for (let j = 0; j < 8; j++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    t[i] = c
  }
  return t
})()

function crc32(buf) {
  let crc = 0xffffffff
  for (const b of buf) crc = CRC_TABLE[(crc ^ b) & 0xff] ^ (crc >>> 8)
  return (crc ^ 0xffffffff) >>> 0
}

function pngChunk(type, data) {
  const typeBytes = Buffer.from(type, 'ascii')
  const len = Buffer.allocUnsafe(4)
  len.writeUInt32BE(data.length)
  const body = Buffer.concat([typeBytes, data])
  const crcBuf = Buffer.allocUnsafe(4)
  crcBuf.writeUInt32BE(crc32(body))
  return Buffer.concat([len, body, crcBuf])
}

/**
 * Renders an Axon-branded icon:
 *   - Background: #0a0f1e (deep navy)
 *   - Outer ring: #00d4ff (cyan) — thick arc covering ~270°
 *   - Inner dot: #00d4ff — small filled circle at center
 *   - Subtle radial glow so it reads well at small sizes
 */
function renderPixel(x, y, size) {
  const cx = size / 2
  const cy = size / 2
  const s = size / 192 // scale factor relative to design at 192px

  const dist = Math.sqrt((x - cx) ** 2 + (y - cy) ** 2)

  // --- outer ring (annulus) ---
  const outerR = size * 0.4
  const innerR = size * 0.28
  const inRing = dist >= innerR && dist <= outerR

  // --- inner dot ---
  const dotR = size * 0.1
  const inDot = dist <= dotR

  // --- gap cutout: top-right arc to make it look like a "C" / signal arc ---
  // gap from ~35° to ~75° (leaving a break in the ring)
  const angle = Math.atan2(y - cy, x - cx) * (180 / Math.PI) + 180 // 0–360
  const inGap = angle >= 30 && angle <= 70

  const isAccent = (inRing && !inGap) || inDot

  // Soft radial gradient for the background (slightly lighter toward center)
  const bgT = Math.max(0, 1 - dist / (size * 0.6))
  const bgR = Math.round(0x0a + bgT * 0x06)
  const bgG = Math.round(0x0f + bgT * 0x08)
  const bgB = Math.round(0x1e + bgT * 0x10)

  if (!isAccent) return [bgR, bgG, bgB]

  // Anti-alias the ring edges
  const outerEdge = Math.max(0, Math.min(1, (outerR - dist) / (s * 1.2) + 0.5))
  const innerEdge = Math.max(0, Math.min(1, (dist - innerR) / (s * 1.2) + 0.5))
  const gapEdge = inRing
    ? Math.max(0, Math.min(1, Math.min(Math.abs(angle - 30), Math.abs(angle - 70)) / 3))
    : 1
  const dotEdge = inDot ? Math.max(0, Math.min(1, (dotR - dist) / (s * 1.0) + 0.5)) : 0

  const alpha = inDot ? dotEdge : inRing ? outerEdge * innerEdge * gapEdge : 0

  // Cyan #00d4ff blended over background
  return [
    Math.round(bgR * (1 - alpha) + 0x00 * alpha),
    Math.round(bgG * (1 - alpha) + 0xd4 * alpha),
    Math.round(bgB * (1 - alpha) + 0xff * alpha),
  ]
}

function generatePng(size) {
  const PNG_SIG = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])

  // IHDR
  const ihdr = Buffer.allocUnsafe(13)
  ihdr.writeUInt32BE(size, 0)
  ihdr.writeUInt32BE(size, 4)
  ihdr[8] = 8 // 8-bit depth
  ihdr[9] = 2 // RGB colour type
  ihdr[10] = 0 // deflate compression
  ihdr[11] = 0 // adaptive filtering
  ihdr[12] = 0 // no interlace

  // Raw image data: 1 filter byte per row + 3 bytes (RGB) per pixel
  const rowSize = 1 + size * 3
  const raw = Buffer.allocUnsafe(size * rowSize)

  for (let y = 0; y < size; y++) {
    const base = y * rowSize
    raw[base] = 0 // filter type: None
    for (let x = 0; x < size; x++) {
      const [r, g, b] = renderPixel(x, y, size)
      const off = base + 1 + x * 3
      raw[off] = r
      raw[off + 1] = g
      raw[off + 2] = b
    }
  }

  const idat = pngChunk('IDAT', deflateSync(raw, { level: 6 }))
  const iend = pngChunk('IEND', Buffer.alloc(0))

  return Buffer.concat([PNG_SIG, pngChunk('IHDR', ihdr), idat, iend])
}

mkdirSync(OUT_DIR, { recursive: true })

for (const size of [192, 512]) {
  const path = resolve(OUT_DIR, `icon-${size}.png`)
  writeFileSync(path, generatePng(size))
  console.log(`✓ ${path}`)
}

console.log('\nDone. Replace public/icons/ with your own assets whenever you like.')
