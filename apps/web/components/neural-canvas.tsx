'use client'

import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react'
import {
  DEFAULT_NEURAL_CANVAS_PROFILE,
  getNeuralCanvasPreset,
  type NeuralCanvasProfile,
  type VisualPresetConfig,
} from '@/lib/pulse/neural-canvas-presets'
import type { ContainerStats } from '@/lib/ws-protocol'

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export interface NeuralCanvasHandle {
  setIntensity: (target: number) => void
  stimulate: (containers: Record<string, ContainerStats>) => void
}

interface NeuralCanvasProps {
  profile?: NeuralCanvasProfile
}

// ---------------------------------------------------------------------------
// Color palette — bioluminescent blue (matches neural.js)
// ---------------------------------------------------------------------------

interface RGB {
  r: number
  g: number
  b: number
}

let COLORS: Record<string, RGB> = {
  core: { r: 210, g: 235, b: 255 },
  bright: { r: 50, g: 160, b: 255 },
  mid: { r: 15, g: 90, b: 210 },
  dim: { r: 8, g: 45, b: 140 },
  faint: { r: 4, g: 20, b: 70 },
}

function applyPalette(preset: VisualPresetConfig) {
  const p = preset.palette
  COLORS = {
    core: p.core,
    bright: p.bright,
    mid: p.mid,
    dim: p.dim,
    faint: p.faint,
  }
}
const CONNECTION_MAX_DIST = 280
const CONNECTION_MAX_DIST_SQ = CONNECTION_MAX_DIST * CONNECTION_MAX_DIST

function rgba(c: RGB, a: number): string {
  return `rgba(${c.r},${c.g},${c.b},${a})`
}

function mixColor(a: RGB, b: RGB, t: number): RGB {
  const v = Math.max(0, Math.min(1, t))
  return {
    r: Math.round(a.r + (b.r - a.r) * v),
    g: Math.round(a.g + (b.g - a.g) * v),
    b: Math.round(a.b + (b.b - a.b) * v),
  }
}

interface RenderAssets {
  neuronOuterGlow: HTMLCanvasElement
  neuronMidGlow: HTMLCanvasElement
  neuronInnerGlow: HTMLCanvasElement
  neuronFlashGlow: HTMLCanvasElement
  actionPotentialGlow: HTMLCanvasElement
  particleGlow: HTMLCanvasElement
  spineGlow: HTMLCanvasElement
}

function createGlowSprite(
  size: number,
  colorStops: Array<{ offset: number; color: string }>,
): HTMLCanvasElement {
  const canvas = document.createElement('canvas')
  canvas.width = size
  canvas.height = size
  const ctx = canvas.getContext('2d')
  if (!ctx) return canvas
  const c = size / 2
  const g = ctx.createRadialGradient(c, c, 0, c, c, c)
  colorStops.forEach((stop) => g.addColorStop(stop.offset, stop.color))
  ctx.fillStyle = g
  ctx.fillRect(0, 0, size, size)
  return canvas
}

function createRenderAssets(preset: VisualPresetConfig): RenderAssets {
  const b = preset.brightness
  const g = preset.glow
  return {
    neuronOuterGlow: createGlowSprite(256, [
      { offset: 0, color: rgba(COLORS.bright, 0.25 * b * g) },
      { offset: 0.3, color: rgba(COLORS.mid, 0.11 * b * g) },
      { offset: 0.65, color: rgba(COLORS.dim, 0.05 * b * g) },
      { offset: 1, color: 'rgba(0,0,0,0)' },
    ]),
    neuronMidGlow: createGlowSprite(192, [
      { offset: 0, color: rgba(COLORS.core, 0.32 * b * g) },
      { offset: 0.35, color: rgba(COLORS.bright, 0.17 * b * g) },
      { offset: 0.75, color: rgba(COLORS.mid, 0.05 * b * g) },
      { offset: 1, color: 'rgba(0,0,0,0)' },
    ]),
    neuronInnerGlow: createGlowSprite(128, [
      { offset: 0, color: rgba(COLORS.core, 0.42 * b * g) },
      { offset: 0.45, color: rgba(COLORS.bright, 0.22 * b * g) },
      { offset: 1, color: 'rgba(0,0,0,0)' },
    ]),
    neuronFlashGlow: createGlowSprite(320, [
      { offset: 0, color: rgba(COLORS.core, 0.36 * b * g) },
      { offset: 0.2, color: rgba(COLORS.bright, 0.2 * b * g) },
      { offset: 0.5, color: rgba(COLORS.mid, 0.08 * b * g) },
      { offset: 1, color: 'rgba(0,0,0,0)' },
    ]),
    actionPotentialGlow: createGlowSprite(48, [
      { offset: 0, color: rgba(COLORS.core, 0.35 * b * g) },
      { offset: 0.35, color: rgba(COLORS.bright, 0.12 * b * g) },
      { offset: 1, color: 'rgba(0,0,0,0)' },
    ]),
    particleGlow: createGlowSprite(64, [
      { offset: 0, color: rgba(COLORS.bright, 0.12 * b * g) },
      { offset: 0.5, color: rgba(COLORS.mid, 0.04 * b * g) },
      { offset: 1, color: 'rgba(0,0,0,0)' },
    ]),
    spineGlow: createGlowSprite(32, [
      { offset: 0, color: rgba(COLORS.bright, 0.24 * b * g) },
      { offset: 1, color: 'rgba(0,0,0,0)' },
    ]),
  }
}

function drawSprite(
  ctx: CanvasRenderingContext2D,
  sprite: HTMLCanvasElement,
  x: number,
  y: number,
  radius: number,
  alpha: number,
) {
  if (alpha <= 0 || radius <= 0) return
  const d = radius * 2
  ctx.save()
  ctx.globalAlpha = alpha
  ctx.drawImage(sprite, x - radius, y - radius, d, d)
  ctx.restore()
}

// ---------------------------------------------------------------------------
// Simplex-like drift (multi-octave sine)
// ---------------------------------------------------------------------------

class SimplexDrift {
  private offsetX: number
  private offsetY: number
  private speed: number

  constructor() {
    this.offsetX = Math.random() * 1000
    this.offsetY = Math.random() * 1000
    this.speed = 0.0003 + Math.random() * 0.0004
  }

  get(time: number): { x: number; y: number } {
    const t = time * this.speed
    const x =
      Math.sin(t + this.offsetX) * 0.3 +
      Math.sin(t * 2.3 + this.offsetX * 1.7) * 0.15 +
      Math.sin(t * 4.1 + this.offsetX * 0.3) * 0.05
    const y =
      Math.sin(t * 0.9 + this.offsetY) * 0.3 +
      Math.sin(t * 1.8 + this.offsetY * 1.3) * 0.15 +
      Math.sin(t * 3.7 + this.offsetY * 0.7) * 0.05
    return { x, y }
  }
}

// ---------------------------------------------------------------------------
// Dendrite — curved, recursively branching structures with spines
// ---------------------------------------------------------------------------

interface Spine {
  t: number
  angle: number
  length: number
}

class Dendrite {
  baseAngle: number
  baseLength: number
  depth: number
  branches: Dendrite[]
  waveOffset: number
  waveSpeed: number
  curvature: number
  startX: number
  startY: number
  endX: number
  endY: number
  cpX: number
  cpY: number
  spines: Spine[]

  constructor(x: number, y: number, angle: number, length: number, depth = 0) {
    this.baseAngle = angle
    this.baseLength = length
    this.depth = depth
    this.branches = []
    this.waveOffset = Math.random() * Math.PI * 2
    this.waveSpeed = 0.0008 + Math.random() * 0.0005
    this.curvature = (Math.random() - 0.5) * length * 0.4

    this.startX = x
    this.startY = y
    this.endX = x + Math.cos(angle) * length
    this.endY = y + Math.sin(angle) * length

    const perpAngle = angle + Math.PI / 2
    this.cpX = (this.startX + this.endX) / 2 + Math.cos(perpAngle) * this.curvature
    this.cpY = (this.startY + this.endY) / 2 + Math.sin(perpAngle) * this.curvature

    // Dendritic spines — LOD: skip for shallow-depth neurons
    this.spines = []
    if (depth < 3) {
      const spineCount = Math.floor(Math.random() * 3) + 1
      for (let i = 0; i < spineCount; i++) {
        this.spines.push({
          t: 0.2 + Math.random() * 0.6,
          angle: (Math.random() - 0.5) * Math.PI,
          length: 3 + Math.random() * 5,
        })
      }
    }

    // Branch deeper with tapering probability
    if (depth < 3 && Math.random() > 0.3 + depth * 0.15) {
      const branchCount = depth < 2 ? Math.floor(Math.random() * 2) + 1 : 1
      for (let i = 0; i < branchCount; i++) {
        const branchAngle = angle + ((Math.random() - 0.5) * Math.PI) / 2.5
        const branchLength = length * (0.45 + Math.random() * 0.25)
        this.branches.push(new Dendrite(this.endX, this.endY, branchAngle, branchLength, depth + 1))
      }
    }
  }

  draw(
    ctx: CanvasRenderingContext2D,
    opacity: number,
    time: number,
    depthScale = 1,
    skipSpines = false,
    assets?: RenderAssets,
  ) {
    const alpha = opacity * (1 - this.depth * 0.18)
    const sway = Math.sin(time * this.waveSpeed + this.waveOffset) * 2
    const cpxs = this.cpX + sway
    const cpys = this.cpY + sway
    const baseWidth = Math.max(0.5, 3.0 - this.depth * 0.7) * depthScale

    // Glow pass — additive
    ctx.save()
    ctx.globalCompositeOperation = 'lighter'
    ctx.beginPath()
    ctx.moveTo(this.startX, this.startY)
    ctx.quadraticCurveTo(cpxs, cpys, this.endX, this.endY)
    ctx.strokeStyle = rgba(COLORS.mid, alpha * 0.12)
    ctx.lineWidth = baseWidth * 5
    ctx.stroke()

    ctx.beginPath()
    ctx.moveTo(this.startX, this.startY)
    ctx.quadraticCurveTo(cpxs, cpys, this.endX, this.endY)
    ctx.strokeStyle = rgba(COLORS.bright, alpha * 0.08)
    ctx.lineWidth = baseWidth * 10
    ctx.stroke()
    ctx.restore()

    // Core dendrite line
    ctx.beginPath()
    ctx.moveTo(this.startX, this.startY)
    ctx.quadraticCurveTo(cpxs, cpys, this.endX, this.endY)
    ctx.strokeStyle = rgba(COLORS.bright, alpha * 0.6)
    ctx.lineWidth = baseWidth
    ctx.stroke()

    // Dendritic spines with glow (LOD: skip for depth < 0.3)
    if (!skipSpines) {
      for (const spine of this.spines) {
        const t = spine.t
        const px = (1 - t) * (1 - t) * this.startX + 2 * (1 - t) * t * cpxs + t * t * this.endX
        const py = (1 - t) * (1 - t) * this.startY + 2 * (1 - t) * t * cpys + t * t * this.endY
        const sx = px + Math.cos(spine.angle) * spine.length * depthScale
        const sy = py + Math.sin(spine.angle) * spine.length * depthScale

        ctx.beginPath()
        ctx.moveTo(px, py)
        ctx.lineTo(sx, sy)
        ctx.strokeStyle = rgba(COLORS.bright, alpha * 0.35)
        ctx.lineWidth = 0.8 * depthScale
        ctx.stroke()

        // Spine head glow
        const headR = 1.5 * depthScale
        if (assets) {
          ctx.save()
          ctx.globalCompositeOperation = 'lighter'
          drawSprite(ctx, assets.spineGlow, sx, sy, headR * 4, alpha * 0.7)
          ctx.restore()
        } else {
          ctx.save()
          ctx.globalCompositeOperation = 'lighter'
          const sg = ctx.createRadialGradient(sx, sy, 0, sx, sy, headR * 4)
          sg.addColorStop(0, rgba(COLORS.bright, alpha * 0.2))
          sg.addColorStop(1, rgba(COLORS.bright, 0))
          ctx.beginPath()
          ctx.arc(sx, sy, headR * 4, 0, Math.PI * 2)
          ctx.fillStyle = sg
          ctx.fill()
          ctx.restore()
        }

        ctx.beginPath()
        ctx.arc(sx, sy, headR, 0, Math.PI * 2)
        ctx.fillStyle = rgba(COLORS.core, alpha * 0.5)
        ctx.fill()
      }
    }

    this.branches.forEach((branch) =>
      branch.draw(ctx, opacity, time, depthScale, skipSpines, assets),
    )
  }

  getTip(): { x: number; y: number } {
    if (this.branches.length === 0) {
      return { x: this.endX, y: this.endY }
    }
    return this.branches[Math.floor(Math.random() * this.branches.length)].getTip()
  }

  updatePosition(newStartX: number, newStartY: number) {
    const dx = newStartX - this.startX
    const dy = newStartY - this.startY
    this.startX = newStartX
    this.startY = newStartY
    this.endX += dx
    this.endY += dy
    this.cpX += dx
    this.cpY += dy
    this.branches.forEach((branch) => branch.updatePosition(this.endX, this.endY))
  }
}

// ---------------------------------------------------------------------------
// Axon — myelin sheaths with Nodes of Ranvier
// ---------------------------------------------------------------------------

interface AxonSegment {
  startX: number
  startY: number
  endX: number
  endY: number
  isMyelin: boolean
}

interface Bouton {
  x: number
  y: number
  radius: number
}

class Axon {
  startX: number
  startY: number
  segments: AxonSegment[]
  terminalX: number
  terminalY: number
  boutons: Bouton[]

  constructor(x: number, y: number) {
    this.startX = x
    this.startY = y

    const baseAngle = Math.random() * Math.PI * 2
    const length = 80 + Math.random() * 120

    this.segments = []
    let currentX = x
    let currentY = y
    let currentAngle = baseAngle

    const segmentCount = 4 + Math.floor(Math.random() * 3)
    const segmentLength = length / segmentCount

    for (let i = 0; i < segmentCount; i++) {
      currentAngle += (Math.random() - 0.5) * 0.35
      const nextX = currentX + Math.cos(currentAngle) * segmentLength
      const nextY = currentY + Math.sin(currentAngle) * segmentLength

      this.segments.push({
        startX: currentX,
        startY: currentY,
        endX: nextX,
        endY: nextY,
        isMyelin: i > 0 && i < segmentCount - 1,
      })

      currentX = nextX
      currentY = nextY
    }

    this.terminalX = currentX
    this.terminalY = currentY

    this.boutons = []
    const boutonCount = 2 + Math.floor(Math.random() * 3)
    for (let i = 0; i < boutonCount; i++) {
      const bAngle = currentAngle + ((Math.random() - 0.5) * Math.PI) / 2
      const bLength = 8 + Math.random() * 15
      this.boutons.push({
        x: currentX + Math.cos(bAngle) * bLength,
        y: currentY + Math.sin(bAngle) * bLength,
        radius: 2.5 + Math.random() * 2,
      })
    }
  }

  draw(ctx: CanvasRenderingContext2D, opacity: number, depthScale = 1) {
    // Glow pass
    ctx.save()
    ctx.globalCompositeOperation = 'lighter'
    this.segments.forEach((segment) => {
      ctx.beginPath()
      ctx.moveTo(segment.startX, segment.startY)
      ctx.lineTo(segment.endX, segment.endY)
      ctx.strokeStyle = rgba(COLORS.mid, opacity * 0.06)
      ctx.lineWidth = (segment.isMyelin ? 12 : 8) * depthScale
      ctx.stroke()
    })
    ctx.restore()

    this.segments.forEach((segment) => {
      if (segment.isMyelin) {
        // Myelin sheath
        ctx.beginPath()
        ctx.moveTo(segment.startX, segment.startY)
        ctx.lineTo(segment.endX, segment.endY)
        ctx.strokeStyle = rgba(COLORS.dim, opacity * 0.35)
        ctx.lineWidth = 4.5 * depthScale
        ctx.stroke()

        // Inner fiber
        ctx.beginPath()
        ctx.moveTo(segment.startX, segment.startY)
        ctx.lineTo(segment.endX, segment.endY)
        ctx.strokeStyle = rgba(COLORS.bright, opacity * 0.45)
        ctx.lineWidth = 1.5 * depthScale
        ctx.stroke()

        // Node of Ranvier
        ctx.save()
        ctx.globalCompositeOperation = 'lighter'
        const nrg = ctx.createRadialGradient(
          segment.endX,
          segment.endY,
          0,
          segment.endX,
          segment.endY,
          8 * depthScale,
        )
        nrg.addColorStop(0, rgba(COLORS.bright, opacity * 0.3))
        nrg.addColorStop(1, rgba(COLORS.bright, 0))
        ctx.beginPath()
        ctx.arc(segment.endX, segment.endY, 8 * depthScale, 0, Math.PI * 2)
        ctx.fillStyle = nrg
        ctx.fill()
        ctx.restore()

        ctx.beginPath()
        ctx.arc(segment.endX, segment.endY, 3 * depthScale, 0, Math.PI * 2)
        ctx.strokeStyle = rgba(COLORS.core, opacity * 0.5)
        ctx.lineWidth = 1
        ctx.stroke()
      } else {
        ctx.beginPath()
        ctx.moveTo(segment.startX, segment.startY)
        ctx.lineTo(segment.endX, segment.endY)
        ctx.strokeStyle = rgba(COLORS.bright, opacity * 0.5)
        ctx.lineWidth = 2 * depthScale
        ctx.stroke()
      }
    })

    // Boutons with glow
    this.boutons.forEach((bouton) => {
      const r = bouton.radius * depthScale
      ctx.beginPath()
      ctx.moveTo(this.terminalX, this.terminalY)
      ctx.lineTo(bouton.x, bouton.y)
      ctx.strokeStyle = rgba(COLORS.bright, opacity * 0.35)
      ctx.lineWidth = 1 * depthScale
      ctx.stroke()

      ctx.save()
      ctx.globalCompositeOperation = 'lighter'
      const bg = ctx.createRadialGradient(bouton.x, bouton.y, 0, bouton.x, bouton.y, r * 5)
      bg.addColorStop(0, rgba(COLORS.bright, opacity * 0.2))
      bg.addColorStop(0.4, rgba(COLORS.mid, opacity * 0.08))
      bg.addColorStop(1, rgba(COLORS.dim, 0))
      ctx.beginPath()
      ctx.arc(bouton.x, bouton.y, r * 5, 0, Math.PI * 2)
      ctx.fillStyle = bg
      ctx.fill()
      ctx.restore()

      ctx.beginPath()
      ctx.arc(bouton.x, bouton.y, r, 0, Math.PI * 2)
      ctx.fillStyle = rgba(COLORS.core, opacity * 0.6)
      ctx.fill()
    })
  }

  updatePosition(newStartX: number, newStartY: number) {
    const dx = newStartX - this.startX
    const dy = newStartY - this.startY
    this.startX = newStartX
    this.startY = newStartY
    this.segments.forEach((s) => {
      s.startX += dx
      s.startY += dy
      s.endX += dx
      s.endY += dy
    })
    this.terminalX += dx
    this.terminalY += dy
    this.boutons.forEach((b) => {
      b.x += dx
      b.y += dy
    })
  }

  getTerminal(): { x: number; y: number } {
    return { x: this.terminalX, y: this.terminalY }
  }
}

// ---------------------------------------------------------------------------
// Neuron — Hodgkin-Huxley inspired membrane potential
// ---------------------------------------------------------------------------

class Neuron {
  x: number
  y: number
  radius: number
  drift: SimplexDrift
  depth: number
  potential: number
  threshold: number
  restingPotential: number
  peakPotential: number
  refractoryTime: number
  refractoryDuration: number
  isFiring: boolean
  firePhase: number
  fireTimer: number
  spontaneousRate: number
  epsp: number
  dendrites: Dendrite[]
  axon: Axon
  outgoingConnections: SynapticConnection[]

  constructor(width: number, height: number) {
    this.x = Math.random() * width
    this.y = Math.random() * height
    this.radius = 8 + Math.random() * 6
    this.drift = new SimplexDrift()
    this.depth = Math.random()

    this.potential = -70
    this.threshold = -55
    this.restingPotential = -70
    this.peakPotential = 40
    this.refractoryTime = 0
    this.refractoryDuration = 80 + Math.random() * 40
    this.isFiring = false
    this.firePhase = 0
    this.fireTimer = 0
    this.spontaneousRate = Math.random() < 0.15 ? 0.001 + Math.random() * 0.002 : 0
    this.epsp = 0

    this.dendrites = []
    const dendriteCount = 4 + Math.floor(Math.random() * 4)
    for (let i = 0; i < dendriteCount; i++) {
      const angle = ((Math.PI * 2) / dendriteCount) * i + (Math.random() - 0.5) * 0.5
      const length = 30 + Math.random() * 50
      this.dendrites.push(new Dendrite(this.x, this.y, angle, length))
    }

    this.axon = new Axon(this.x, this.y)
    this.outgoingConnections = []
  }

  receiveSignal(strength = 15) {
    if (this.refractoryTime > 0) return
    this.epsp += strength
  }

  update(time: number, dt: number, width: number, height: number, driftScale = 1) {
    const d = this.drift.get(time)
    this.x += d.x * driftScale
    this.y += d.y * driftScale

    const margin = 100
    if (this.x < -margin) this.x = width + margin
    if (this.x > width + margin) this.x = -margin
    if (this.y < -margin) this.y = height + margin
    if (this.y > height + margin) this.y = -margin

    this.dendrites.forEach((dd) => dd.updatePosition(this.x, this.y))
    this.axon.updatePosition(this.x, this.y)

    if (this.refractoryTime > 0) {
      this.refractoryTime -= dt
      this.potential += (this.restingPotential - 5 - this.potential) * 0.05
      this.epsp = 0
      return
    }

    if (this.spontaneousRate > 0 && Math.random() < this.spontaneousRate) {
      this.epsp += 20
    }

    this.potential += this.epsp * 0.5
    this.epsp *= 0.85
    this.potential += (this.restingPotential - this.potential) * 0.02

    if (!this.isFiring && this.potential >= this.threshold) {
      this.isFiring = true
      this.firePhase = 1
      this.fireTimer = 0
    }

    if (this.isFiring) {
      this.fireTimer += dt
      if (this.firePhase === 1) {
        this.potential += (this.peakPotential - this.potential) * 0.3
        if (this.potential > this.peakPotential - 5) {
          this.firePhase = 2
        }
      } else if (this.firePhase === 2) {
        this.potential += (this.restingPotential - 10 - this.potential) * 0.15
        if (this.potential < this.restingPotential) {
          this.firePhase = 0
          this.isFiring = false
          this.refractoryTime = this.refractoryDuration
          this.potential = this.restingPotential - 8
        }
      }
    }
  }

  draw(
    ctx: CanvasRenderingContext2D,
    time: number,
    options: {
      fineDetail: boolean
      allowSpines: boolean
      assets: RenderAssets
      visual: VisualPresetConfig
    },
  ) {
    const { fineDetail, allowSpines, assets, visual } = options
    const potentialNorm = Math.max(
      0,
      Math.min(
        1,
        (this.potential - this.restingPotential) / (this.peakPotential - this.restingPotential),
      ),
    )

    const flicker = Math.sin(time * 0.002 + this.x * 0.01) * 0.05 + 0.95
    const baseOpacity = (0.3 + potentialNorm * 0.7) * flicker * visual.brightness

    const ds = 0.4 + this.depth * 0.6
    const da = 0.25 + this.depth * 0.75

    // LOD: skip spines for depth < 0.3
    const skipSpines = this.depth < 0.3 || !allowSpines

    this.axon.draw(ctx, baseOpacity * da, ds)
    this.dendrites.forEach((d) => d.draw(ctx, baseOpacity * da, time, ds, skipSpines, assets))

    const r = this.radius * ds
    const x = this.x
    const y = this.y
    const activatedCore = mixColor(
      COLORS.bright,
      COLORS.core,
      Math.min(1, potentialNorm * 0.8 * visual.glow),
    )

    // Volumetric glow (additive)
    ctx.save()
    ctx.globalCompositeOperation = 'lighter'

    // Outermost bloom
    const outerR = r * (10 + potentialNorm * 8)
    const oA = (0.02 + potentialNorm * 0.06) * da
    drawSprite(ctx, assets.neuronOuterGlow, x, y, outerR, oA)

    // Mid bloom
    const midR = r * (5 + potentialNorm * 3)
    const mA = (0.06 + potentialNorm * 0.12) * da
    drawSprite(ctx, assets.neuronMidGlow, x, y, midR, mA)

    // Inner glow
    const innerR = r * (2.5 + potentialNorm * 1.5)
    const iA = (0.12 + potentialNorm * 0.3) * da
    drawSprite(ctx, assets.neuronInnerGlow, x, y, innerR, iA)

    // Firing flash
    if (this.isFiring) {
      const flashR = r * (14 + potentialNorm * 10)
      drawSprite(ctx, assets.neuronFlashGlow, x, y, flashR, 0.32 * potentialNorm * da)
    }

    ctx.restore()

    // Soma body
    const offX = -r * 0.25
    const offY = -r * 0.25

    const somaG = ctx.createRadialGradient(x + offX, y + offY, r * 0.1, x, y, r)
    somaG.addColorStop(0, rgba(activatedCore, (0.3 + potentialNorm * 0.4) * da))
    somaG.addColorStop(0.4, rgba(COLORS.bright, (0.18 + potentialNorm * 0.2) * da))
    somaG.addColorStop(0.8, rgba(COLORS.mid, (0.1 + potentialNorm * 0.1) * da))
    somaG.addColorStop(1, rgba(COLORS.dim, 0.05 * da))
    ctx.beginPath()
    ctx.arc(x, y, r, 0, Math.PI * 2)
    ctx.fillStyle = somaG
    ctx.fill()

    // Membrane ring
    ctx.beginPath()
    ctx.arc(x, y, r + 0.5, 0, Math.PI * 2)
    ctx.strokeStyle = rgba(COLORS.bright, (0.15 + potentialNorm * 0.3) * da)
    ctx.lineWidth = 1.2 * ds
    ctx.stroke()

    // Internal texture (only foreground neurons)
    if (fineDetail && this.depth > 0.4) {
      ctx.save()
      ctx.globalAlpha = (0.06 + potentialNorm * 0.08) * da
      for (let k = 0; k < 5; k++) {
        const angle1 = (k / 5) * Math.PI * 2 + time * 0.0001
        const angle2 = angle1 + Math.PI * 0.6
        ctx.beginPath()
        ctx.moveTo(x + Math.cos(angle1) * r * 0.7, y + Math.sin(angle1) * r * 0.7)
        ctx.quadraticCurveTo(
          x + Math.cos((angle1 + angle2) / 2) * r * 0.3,
          y + Math.sin((angle1 + angle2) / 2) * r * 0.3,
          x + Math.cos(angle2) * r * 0.7,
          y + Math.sin(angle2) * r * 0.7,
        )
        ctx.strokeStyle = rgba(COLORS.bright, 0.4)
        ctx.lineWidth = 0.5
        ctx.stroke()
      }
      ctx.restore()
    }

    // Nucleus
    const nucR = r * 0.4
    const nucG = ctx.createRadialGradient(x + offX * 0.3, y + offY * 0.3, 0, x, y, nucR)
    nucG.addColorStop(0, rgba(activatedCore, (0.55 + potentialNorm * 0.4) * da))
    nucG.addColorStop(0.5, rgba(COLORS.bright, (0.25 + potentialNorm * 0.2) * da))
    nucG.addColorStop(1, rgba(COLORS.mid, 0))
    ctx.beginPath()
    ctx.arc(x, y, nucR, 0, Math.PI * 2)
    ctx.fillStyle = nucG
    ctx.fill()
  }

  getRandomDendriteTip(): { x: number; y: number } {
    const d = this.dendrites[Math.floor(Math.random() * this.dendrites.length)]
    return d.getTip()
  }
}

// ---------------------------------------------------------------------------
// SynapticConnection — fiber between axon terminal and dendrite tip
// ---------------------------------------------------------------------------

class SynapticConnection {
  preNeuron: Neuron
  postNeuron: Neuron
  preTerminal: { x: number; y: number }
  dendriteTip: { x: number; y: number }
  strength: number
  baseAlpha: number
  bucket: 0 | 1 | 2

  constructor(
    preNeuron: Neuron,
    postNeuron: Neuron,
    preTerminal: { x: number; y: number },
    dendriteTip: { x: number; y: number },
  ) {
    this.preNeuron = preNeuron
    this.postNeuron = postNeuron
    this.preTerminal = preTerminal
    this.dendriteTip = dendriteTip
    this.strength = 0.3 + Math.random() * 0.7

    const dx = dendriteTip.x - preTerminal.x
    const dy = dendriteTip.y - preTerminal.y
    const distNorm = Math.min(1, Math.sqrt(dx * dx + dy * dy) / CONNECTION_MAX_DIST)
    this.baseAlpha = (1 - distNorm) * 0.18 * this.strength
    this.bucket = this.baseAlpha > 0.12 ? 0 : this.baseAlpha > 0.06 ? 1 : 2
  }
}

// ---------------------------------------------------------------------------
// ActionPotential — traveling signal dot with glow
// ---------------------------------------------------------------------------

class ActionPotential {
  neuron: Neuron
  connection: SynapticConnection
  segments: AxonSegment[]
  phase: 'axon' | 'synapse' | 'dendrite'
  progress: number
  currentSegment: number
  active: boolean
  synapseTimer: number
  baseSpeed: number

  constructor(neuron: Neuron, connection: SynapticConnection) {
    this.neuron = neuron
    this.connection = connection
    this.segments = neuron.axon.segments
    this.phase = 'axon'
    this.progress = 0
    this.currentSegment = 0
    this.active = true
    this.synapseTimer = 0
    this.baseSpeed = 0.02 + Math.random() * 0.015
  }

  update(_dt: number) {
    if (this.phase === 'axon') {
      const seg = this.segments[this.currentSegment]
      this.progress += seg.isMyelin ? this.baseSpeed * 3 : this.baseSpeed
      if (this.progress >= 1) {
        this.currentSegment++
        this.progress = 0
        if (this.currentSegment >= this.segments.length) {
          this.phase = 'synapse'
          this.synapseTimer = 0
        }
      }
    } else if (this.phase === 'synapse') {
      this.synapseTimer += 0.04
      if (this.synapseTimer >= 1) {
        this.connection.postNeuron.receiveSignal(10 * this.connection.strength)
        this.phase = 'dendrite'
        this.progress = 0
      }
    } else if (this.phase === 'dendrite') {
      this.progress += this.baseSpeed * 1.5
      if (this.progress >= 1) this.active = false
    }
  }

  draw(ctx: CanvasRenderingContext2D, assets: RenderAssets, withGlow = true) {
    if (!this.active) return

    let x: number | undefined
    let y: number | undefined

    if (this.phase === 'axon') {
      const seg = this.segments[this.currentSegment]
      const t = Math.min(this.progress, 1)
      x = seg.startX + (seg.endX - seg.startX) * t
      y = seg.startY + (seg.endY - seg.startY) * t
    } else if (this.phase === 'synapse') {
      const pre = this.connection.preTerminal
      const post = this.connection.dendriteTip
      x = pre.x + (post.x - pre.x) * this.synapseTimer
      y = pre.y + (post.y - pre.y) * this.synapseTimer
    } else if (this.phase === 'dendrite') {
      const start = this.connection.dendriteTip
      const end = this.connection.postNeuron
      x = start.x + (end.x - start.x) * this.progress
      y = start.y + (end.y - start.y) * this.progress
    }

    if (x === undefined || y === undefined) return

    // Glow halo
    if (withGlow) {
      ctx.save()
      ctx.globalCompositeOperation = 'lighter'
      drawSprite(ctx, assets.actionPotentialGlow, x, y, 10, 0.85)
      ctx.restore()
    }

    // Core dot
    ctx.beginPath()
    ctx.arc(x, y, 2.5, 0, Math.PI * 2)
    ctx.fillStyle = rgba(COLORS.core, 0.95)
    ctx.fill()
  }
}

// ---------------------------------------------------------------------------
// BackgroundParticle — bokeh field
// ---------------------------------------------------------------------------

class BackgroundParticle {
  x: number
  y: number
  z: number
  baseSize: number
  brightness: number
  drift: SimplexDrift
  pulseOffset: number
  pulseSpeed: number

  constructor(width: number, height: number) {
    this.x = Math.random() * width
    this.y = Math.random() * height
    this.z = Math.random()
    this.baseSize = 0.4 + Math.random() * 1.8
    this.brightness = 0.08 + this.z * 0.4 + Math.random() * 0.15
    this.drift = new SimplexDrift()
    this.pulseOffset = Math.random() * Math.PI * 2
    this.pulseSpeed = 0.0008 + Math.random() * 0.0015
  }

  update(time: number, width: number, height: number, driftScale = 1) {
    const d = this.drift.get(time)
    this.x += d.x * 0.2 * driftScale
    this.y += d.y * 0.2 * driftScale
    if (this.x < -20) this.x = width + 20
    if (this.x > width + 20) this.x = -20
    if (this.y < -20) this.y = height + 20
    if (this.y > height + 20) this.y = -20
  }

  draw(
    ctx: CanvasRenderingContext2D,
    time: number,
    assets: RenderAssets,
    withGlow = true,
    snap = false,
  ) {
    const pulse = Math.sin(time * this.pulseSpeed + this.pulseOffset) * 0.2 + 0.8
    const alpha = this.brightness * pulse
    const sz = this.baseSize * (0.5 + this.z * 0.5) * pulse
    const x = snap ? Math.round(this.x) : this.x
    const y = snap ? Math.round(this.y) : this.y

    if (withGlow && this.brightness > 0.2) {
      ctx.save()
      ctx.globalCompositeOperation = 'lighter'
      drawSprite(ctx, assets.particleGlow, x, y, sz * 6, alpha * 0.9)
      ctx.restore()
    }

    ctx.beginPath()
    ctx.arc(x, y, sz, 0, Math.PI * 2)
    ctx.fillStyle = rgba(COLORS.core, alpha * 0.7)
    ctx.fill()
  }
}

// ---------------------------------------------------------------------------
// Batched connection renderer
// ---------------------------------------------------------------------------

interface ConnectionBuckets {
  strong: SynapticConnection[]
  medium: SynapticConnection[]
  faint: SynapticConnection[]
}

function buildConnectionBuckets(conns: SynapticConnection[]): ConnectionBuckets {
  const strong: SynapticConnection[] = []
  const medium: SynapticConnection[] = []
  const faint: SynapticConnection[] = []
  for (let i = 0; i < conns.length; i++) {
    const c = conns[i]
    if (c.bucket === 0) strong.push(c)
    else if (c.bucket === 1) medium.push(c)
    else faint.push(c)
  }
  return { strong, medium, faint }
}

function drawConnections(
  ctx: CanvasRenderingContext2D,
  buckets: ConnectionBuckets,
  neuralIntensity: number,
  time: number,
  preset: VisualPresetConfig,
  stride = 1,
) {
  const grouped = [buckets.strong, buckets.medium, buckets.faint]

  const alphas = [0.12, 0.06, 0.03]
  const widths = [0.6, 0.4, 0.3]
  const glowWidths = [3, 2, 1.5]
  const pulse = 0.9 + 0.1 * Math.sin(time * 0.004 * preset.pulse)
  const boost = (1 + neuralIntensity * 2) * pulse
  const glowColor = mixColor(
    COLORS.dim,
    COLORS.mid,
    Math.min(1, neuralIntensity * 0.75 * preset.glow),
  )
  const fiberColor = mixColor(
    COLORS.mid,
    COLORS.core,
    Math.min(1, neuralIntensity * 0.5 * preset.brightness),
  )

  // Glow pass (additive)
  ctx.save()
  ctx.globalCompositeOperation = 'lighter'
  for (let b = 0; b < 3; b++) {
    if (grouped[b].length === 0) continue
    ctx.strokeStyle = rgba(glowColor, Math.min(alphas[b] * boost * 0.5 * preset.brightness, 0.2))
    ctx.lineWidth = glowWidths[b]
    ctx.beginPath()
    for (let i = 0; i < grouped[b].length; i++) {
      if (stride > 1 && i % stride !== 0) continue
      const c = grouped[b][i]
      ctx.moveTo(c.preTerminal.x, c.preTerminal.y)
      ctx.lineTo(c.dendriteTip.x, c.dendriteTip.y)
    }
    ctx.stroke()
  }
  ctx.restore()

  // Crisp fiber pass
  for (let b = 0; b < 3; b++) {
    if (grouped[b].length === 0) continue
    ctx.strokeStyle = rgba(fiberColor, Math.min(alphas[b] * boost * preset.brightness, 0.32))
    ctx.lineWidth = widths[b]
    ctx.beginPath()
    for (let i = 0; i < grouped[b].length; i++) {
      if (stride > 1 && i % stride !== 0) continue
      const c = grouped[b][i]
      ctx.moveTo(c.preTerminal.x, c.preTerminal.y)
      ctx.lineTo(c.dendriteTip.x, c.dendriteTip.y)
    }
    ctx.stroke()
  }
}

// ---------------------------------------------------------------------------
// Initialization helper — creates neurons, connections, particles
// ---------------------------------------------------------------------------

interface AnimState {
  neurons: Neuron[]
  connections: SynapticConnection[]
  connectionBuckets: ConnectionBuckets
  signals: ActionPotential[]
  particles: BackgroundParticle[]
  densityLayer: HTMLCanvasElement
  backgroundLayer: HTMLCanvasElement
  backgroundNeedsRefresh: boolean
  backgroundInterval: number
  visual: VisualPresetConfig
  renderAssets: RenderAssets
  intensity: number
  targetIntensity: number
  burstCooldown: number
  frameId: number
  lastTime: number
  lastRenderTime: number
  fpsEma: number
  frameCount: number
  width: number
  height: number
  targetFrameMs: number
  isVisible: boolean
}

function createDensityLayer(
  width: number,
  height: number,
  reducedMotion: boolean,
  preset: VisualPresetConfig,
): HTMLCanvasElement {
  const layer = document.createElement('canvas')
  layer.width = width
  layer.height = height
  const layerCtx = layer.getContext('2d')
  if (!layerCtx) return layer

  const area = width * height
  const baseCount = (reducedMotion ? 0.12 : 0.2) * preset.density
  const dotCount = Math.max(120, Math.round((area / 10000) * baseCount * 100))

  for (let i = 0; i < dotCount; i++) {
    const x = Math.random() * width
    const y = Math.random() * height
    const r = Math.random() < 0.85 ? 0.35 + Math.random() * 0.9 : 0.8 + Math.random() * 1.2
    const alpha = (0.02 + Math.random() * 0.055) * preset.brightness
    layerCtx.beginPath()
    layerCtx.arc(x, y, r, 0, Math.PI * 2)
    layerCtx.fillStyle = rgba(COLORS.faint, alpha)
    layerCtx.fill()
  }

  return layer
}

function createBackgroundLayer(width: number, height: number): HTMLCanvasElement {
  const layer = document.createElement('canvas')
  layer.width = width
  layer.height = height
  return layer
}

function createAnimState(
  width: number,
  height: number,
  options: {
    neuronCount: number
    particleCount: number
    targetFrameMs: number
    reducedMotion: boolean
    visual: VisualPresetConfig
  },
): AnimState {
  const { neuronCount, particleCount, targetFrameMs, reducedMotion, visual } = options

  const particles: BackgroundParticle[] = []
  for (let i = 0; i < particleCount; i++) {
    particles.push(new BackgroundParticle(width, height))
  }

  const neurons: Neuron[] = []
  for (let i = 0; i < neuronCount; i++) {
    neurons.push(new Neuron(width, height))
  }

  // Build synaptic web
  const connections: SynapticConnection[] = []
  neurons.forEach((neuron, i) => {
    const terminal = neuron.axon.getTerminal()
    const endpoints = [terminal, ...neuron.axon.boutons.map((b) => ({ x: b.x, y: b.y }))]

    neurons.forEach((target, j) => {
      if (i === j) return
      endpoints.forEach((ep) => {
        target.dendrites.forEach((dendrite) => {
          const tip = dendrite.getTip()
          const dx = ep.x - tip.x
          const dy = ep.y - tip.y
          const distSq = dx * dx + dy * dy

          if (distSq < CONNECTION_MAX_DIST_SQ && Math.random() > 0.45) {
            const conn = new SynapticConnection(neuron, target, ep, tip)
            connections.push(conn)
            neuron.outgoingConnections.push(conn)
          }
        })
      })
    })
  })

  // Sort by depth for painter's algorithm (back to front)
  neurons.sort((a, b) => a.depth - b.depth)

  const connectionBuckets = buildConnectionBuckets(connections)

  return {
    neurons,
    connections,
    connectionBuckets,
    signals: [],
    particles,
    densityLayer: createDensityLayer(width, height, reducedMotion, visual),
    backgroundLayer: createBackgroundLayer(width, height),
    backgroundNeedsRefresh: true,
    backgroundInterval: Math.max(
      1,
      reducedMotion ? visual.backgroundInterval + 1 : visual.backgroundInterval,
    ),
    visual,
    renderAssets: createRenderAssets(visual),
    intensity: 0,
    targetIntensity: 0,
    burstCooldown: 0,
    frameId: 0,
    lastTime: 0,
    lastRenderTime: 0,
    fpsEma: 60,
    frameCount: 0,
    width,
    height,
    targetFrameMs,
    isVisible: true,
  }
}

// ---------------------------------------------------------------------------
// React component
// ---------------------------------------------------------------------------

const NeuralCanvas = forwardRef<NeuralCanvasHandle, NeuralCanvasProps>(function NeuralCanvas(
  { profile = DEFAULT_NEURAL_CANVAS_PROFILE },
  ref,
) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const stateRef = useRef<AnimState | null>(null)

  useImperativeHandle(ref, () => ({
    setIntensity(target: number) {
      if (stateRef.current) {
        stateRef.current.targetIntensity = Math.max(0, Math.min(1, target))
      }
    },
    stimulate(containers: Record<string, ContainerStats>) {
      if (!stateRef.current) return
      // Map container CPU usage to neural intensity
      const values = Object.values(containers)
      if (values.length === 0) return
      const avgCpu = values.reduce((sum, c) => sum + c.cpu_percent, 0) / values.length
      // Scale: 0-100% CPU -> 0-1 intensity
      stateRef.current.targetIntensity = Math.max(0, Math.min(1, avgCpu / 100))
      // Also directly stimulate random neurons proportional to CPU load
      const state = stateRef.current
      const stimCount = Math.floor(avgCpu / 20)
      for (let i = 0; i < stimCount && i < state.neurons.length; i++) {
        const idx = Math.floor(Math.random() * state.neurons.length)
        state.neurons[idx].receiveSignal(20 + avgCpu * 0.3)
      }
    },
  }))

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return
    const visual = getNeuralCanvasPreset(profile)
    applyPalette(visual)

    const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    const hardwareCores =
      typeof navigator.hardwareConcurrency === 'number' ? navigator.hardwareConcurrency : 8
    const deviceMemory =
      typeof (navigator as Navigator & { deviceMemory?: number }).deviceMemory === 'number'
        ? ((navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? 8)
        : 8
    const lowCoreCount = hardwareCores <= 4
    const megapixels = (window.innerWidth * window.innerHeight) / 1_000_000
    const performanceScore = Math.max(
      0.55,
      Math.min(1.25, hardwareCores / 8) * Math.max(0.7, Math.min(1.2, deviceMemory / 8)),
    )
    const pixelPenalty = megapixels > 3 ? 0.82 : megapixels > 2 ? 0.9 : 1
    const qualityScale = Math.max(
      0.52,
      Math.min(1.1, (reducedMotion ? 0.72 : 1) * performanceScore * pixelPenalty),
    )

    const maxDpr =
      reducedMotion || lowCoreCount || megapixels > 3 ? 1 : megapixels > 2 ? 1.15 : 1.25
    const targetFrameMs = reducedMotion
      ? 1000 / 35
      : qualityScale < 0.75
        ? 1000 / 48
        : lowCoreCount
          ? 1000 / 52
          : 1000 / 60
    const viewportScale = Math.min(
      1.15,
      Math.max(0.7, (window.innerWidth * window.innerHeight) / (1920 * 1080)),
    )
    const neuronCount = Math.max(
      26,
      Math.round((reducedMotion ? 42 : 62) * viewportScale * qualityScale),
    )
    const particleCount = Math.max(
      110,
      Math.round((reducedMotion ? 180 : 320) * viewportScale * qualityScale),
    )

    // Size to viewport with DPR scaling for crisp rendering on high-DPI displays
    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, maxDpr)
      const w = window.innerWidth
      const h = window.innerHeight
      canvas.width = w * dpr
      canvas.height = h * dpr
      canvas.style.width = `${w}px`
      canvas.style.height = `${h}px`
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      if (stateRef.current) {
        stateRef.current.width = w
        stateRef.current.height = h
        stateRef.current.densityLayer = createDensityLayer(w, h, reducedMotion, visual)
        stateRef.current.backgroundLayer = createBackgroundLayer(w, h)
        stateRef.current.backgroundNeedsRefresh = true
      }
    }
    resize()

    // Debounced resize
    let resizeTimeout: ReturnType<typeof setTimeout>
    const onResize = () => {
      clearTimeout(resizeTimeout)
      resizeTimeout = setTimeout(resize, 100)
    }
    window.addEventListener('resize', onResize)

    // Initialize state
    const state = createAnimState(window.innerWidth, window.innerHeight, {
      neuronCount,
      particleCount,
      targetFrameMs,
      reducedMotion,
      visual,
    })
    state.isVisible = document.visibilityState === 'visible'
    stateRef.current = state
    const onVisibilityChange = () => {
      if (!stateRef.current) return
      stateRef.current.isVisible = document.visibilityState === 'visible'
    }
    document.addEventListener('visibilitychange', onVisibilityChange)

    // Animation loop
    const animate = (currentTime: number) => {
      if (!state.isVisible) {
        state.lastTime = currentTime
        state.lastRenderTime = currentTime
        state.frameId = requestAnimationFrame(animate)
        return
      }

      const dt = Math.min(currentTime - state.lastTime, 50)
      state.lastTime = currentTime
      if (currentTime - state.lastRenderTime < state.targetFrameMs) {
        state.frameId = requestAnimationFrame(animate)
        return
      }
      state.lastRenderTime = currentTime
      state.frameCount++

      const fps = dt > 0 ? 1000 / dt : 60
      state.fpsEma = state.fpsEma * 0.9 + fps * 0.1
      const degraded = reducedMotion || state.fpsEma < 44
      const medium = state.fpsEma < 52
      const particleStride = degraded ? 3 : medium ? 2 : 1
      const connectionStride = degraded ? 3 : medium ? 2 : 1
      const signalCap = degraded ? 36 : medium ? 48 : 60
      const fineNeuronBudget = degraded ? 0 : medium ? 10 : 18
      const spineNeuronBudget = degraded ? 0 : medium ? 8 : 16

      // Smooth intensity response: quick ramp-up, calm nonlinear recovery.
      const intensityDelta = state.targetIntensity - state.intensity
      const lerpRate =
        intensityDelta > 0
          ? 0.12
          : 0.008 +
            Math.min(1, Math.abs(intensityDelta)) ** 2 *
              (0.02 / Math.max(0.5, state.visual.calmRecovery))
      state.intensity += (state.targetIntensity - state.intensity) * lerpRate
      state.burstCooldown = Math.max(0, state.burstCooldown - dt)

      const w = state.width
      const h = state.height

      // Clear frame (matches original neural.js)
      ctx.clearRect(0, 0, w, h)

      // Update particles first so both background and foreground layers stay in sync.
      state.particles.forEach((p) => {
        const parallax = 0.9 + p.z * 0.8 * state.visual.parallaxDepth
        p.update(currentTime, w, h, parallax)
      })

      // Layered background refresh every N frames.
      if (state.backgroundNeedsRefresh || state.frameCount % state.backgroundInterval === 0) {
        const bgCtx = state.backgroundLayer.getContext('2d')
        if (bgCtx) {
          bgCtx.clearRect(0, 0, w, h)
          bgCtx.drawImage(state.densityLayer, 0, 0, w, h)

          const cx = w / 2
          const cy = h / 2
          const ambR = Math.max(w, h) * 0.5
          const ambG = bgCtx.createRadialGradient(cx, cy, 0, cx, cy, ambR)
          ambG.addColorStop(
            0,
            rgba(
              state.visual.palette.backgroundInner,
              (0.05 + state.intensity * 0.035) * state.visual.brightness,
            ),
          )
          ambG.addColorStop(0.5, rgba(state.visual.palette.dim, 0.02 * state.visual.brightness))
          ambG.addColorStop(1, rgba(state.visual.palette.backgroundOuter, 0))
          bgCtx.fillStyle = ambG
          bgCtx.fillRect(0, 0, w, h)

          state.particles.forEach((p, index) => {
            if (index % particleStride !== 0) return
            if (p.z < 0.5) {
              p.draw(bgCtx, currentTime, state.renderAssets, !degraded, degraded)
            }
          })
        }
        state.backgroundNeedsRefresh = false
      }
      ctx.drawImage(state.backgroundLayer, 0, 0, w, h)

      const masterAlpha = 0.94 + state.intensity * 0.08 * state.visual.brightness
      ctx.save()
      ctx.globalAlpha = masterAlpha

      // Connection fibers
      drawConnections(
        ctx,
        state.connectionBuckets,
        state.intensity,
        currentTime,
        state.visual,
        connectionStride,
      )

      // Update and draw neurons (sorted back-to-front)
      let fineUsed = 0
      let spineUsed = 0
      state.neurons.forEach((neuron) => {
        const neuronParallax = 0.8 + neuron.depth * 0.9 * state.visual.parallaxDepth
        neuron.update(currentTime, dt, w, h, neuronParallax)
        const allowFineDetail = !degraded && neuron.depth > 0.45 && fineUsed < fineNeuronBudget
        const allowSpines = !degraded && neuron.depth > 0.35 && spineUsed < spineNeuronBudget
        neuron.draw(ctx, currentTime, {
          fineDetail: allowFineDetail,
          allowSpines,
          assets: state.renderAssets,
          visual: state.visual,
        })
        if (allowFineDetail) fineUsed++
        if (allowSpines) spineUsed++

        // Fire action potentials
        if (neuron.isFiring && neuron.firePhase === 1 && neuron.fireTimer < dt * 2) {
          const outgoing = neuron.outgoingConnections
          const maxSignals = Math.max(1, 2 + Math.floor((state.intensity * 5) / connectionStride))
          let sent = 0
          for (let c = 0; c < outgoing.length && sent < maxSignals; c++) {
            if (Math.random() < outgoing[c].strength * 0.4) {
              state.signals.push(new ActionPotential(neuron, outgoing[c]))
              sent++
            }
          }
        }

        // Intensity-driven spontaneous activity
        if (state.intensity > 0.1 && !neuron.isFiring && neuron.refractoryTime <= 0) {
          if (Math.random() < state.intensity * 0.025 * state.visual.activity) {
            neuron.epsp += 25
          }
        }
      })

      if (
        state.intensity >= state.visual.burstThreshold &&
        state.burstCooldown <= 0 &&
        state.neurons.length > 0
      ) {
        const focusIndex = Math.floor(Math.random() * state.neurons.length)
        const focus = state.neurons[focusIndex]
        const burstRadiusSq = (180 * state.visual.burstStrength) ** 2
        let excited = 0
        for (let i = 0; i < state.neurons.length && excited < 16; i++) {
          const n = state.neurons[i]
          const dx = n.x - focus.x
          const dy = n.y - focus.y
          const near = dx * dx + dy * dy < burstRadiusSq
          if (near && Math.random() < 0.55 * state.visual.burstStrength) {
            n.receiveSignal(22 * state.visual.burstStrength)
            excited++
          }
        }
        state.burstCooldown =
          Math.max(260, 980 - state.intensity * 500) / Math.max(0.85, state.visual.burstStrength)
      }

      // Action potentials
      for (let i = state.signals.length - 1; i >= 0; i--) {
        state.signals[i].update(dt)
        state.signals[i].draw(ctx, state.renderAssets, !degraded)
        if (!state.signals[i].active) state.signals.splice(i, 1)
      }
      while (state.signals.length > signalCap) state.signals.shift()

      // Foreground particles (z >= 0.5)
      state.particles.forEach((p, index) => {
        if (index % particleStride !== 0) return
        if (p.z >= 0.5) p.draw(ctx, currentTime, state.renderAssets, !degraded, degraded)
      })
      ctx.restore()

      state.frameId = requestAnimationFrame(animate)
    }

    state.frameId = requestAnimationFrame(animate)

    // Cleanup
    return () => {
      cancelAnimationFrame(state.frameId)
      window.removeEventListener('resize', onResize)
      document.removeEventListener('visibilitychange', onVisibilityChange)
      clearTimeout(resizeTimeout)
      stateRef.current = null
    }
  }, [profile])

  return (
    <canvas ref={canvasRef} className="fixed inset-0 z-0" style={{ willChange: 'transform' }} />
  )
})

NeuralCanvas.displayName = 'NeuralCanvas'

export default NeuralCanvas
