'use client'

import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react'
import type { ContainerStats } from '@/lib/ws-protocol'

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export interface NeuralCanvasHandle {
  setIntensity: (target: number) => void
  stimulate: (containers: Record<string, ContainerStats>) => void
}

// ---------------------------------------------------------------------------
// Color palette — bioluminescent blue (matches neural.js)
// ---------------------------------------------------------------------------

interface RGB {
  r: number
  g: number
  b: number
}

const COLORS: Record<string, RGB> = {
  core: { r: 210, g: 235, b: 255 },
  bright: { r: 50, g: 160, b: 255 },
  mid: { r: 15, g: 90, b: 210 },
  dim: { r: 8, g: 45, b: 140 },
  faint: { r: 4, g: 20, b: 70 },
}

function rgba(c: RGB, a: number): string {
  return `rgba(${c.r},${c.g},${c.b},${a})`
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

        ctx.beginPath()
        ctx.arc(sx, sy, headR, 0, Math.PI * 2)
        ctx.fillStyle = rgba(COLORS.core, alpha * 0.5)
        ctx.fill()
      }
    }

    this.branches.forEach((branch) => branch.draw(ctx, opacity, time, depthScale, skipSpines))
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

  update(time: number, dt: number, width: number, height: number) {
    const d = this.drift.get(time)
    this.x += d.x
    this.y += d.y

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

  draw(ctx: CanvasRenderingContext2D, time: number) {
    const potentialNorm = Math.max(
      0,
      Math.min(
        1,
        (this.potential - this.restingPotential) / (this.peakPotential - this.restingPotential),
      ),
    )

    const flicker = Math.sin(time * 0.002 + this.x * 0.01) * 0.05 + 0.95
    const baseOpacity = (0.3 + potentialNorm * 0.7) * flicker

    const ds = 0.4 + this.depth * 0.6
    const da = 0.25 + this.depth * 0.75

    // LOD: skip spines for depth < 0.3
    const skipSpines = this.depth < 0.3

    this.axon.draw(ctx, baseOpacity * da, ds)
    this.dendrites.forEach((d) => d.draw(ctx, baseOpacity * da, time, ds, skipSpines))

    const r = this.radius * ds
    const x = this.x
    const y = this.y

    // Volumetric glow (additive)
    ctx.save()
    ctx.globalCompositeOperation = 'lighter'

    // Outermost bloom
    const outerR = r * (10 + potentialNorm * 8)
    const outerG = ctx.createRadialGradient(x, y, 0, x, y, outerR)
    const oA = (0.02 + potentialNorm * 0.06) * da
    outerG.addColorStop(0, rgba(COLORS.bright, oA))
    outerG.addColorStop(0.25, rgba(COLORS.mid, oA * 0.5))
    outerG.addColorStop(0.6, rgba(COLORS.dim, oA * 0.15))
    outerG.addColorStop(1, 'rgba(0,0,0,0)')
    ctx.beginPath()
    ctx.arc(x, y, outerR, 0, Math.PI * 2)
    ctx.fillStyle = outerG
    ctx.fill()

    // Mid bloom
    const midR = r * (5 + potentialNorm * 3)
    const midG = ctx.createRadialGradient(x, y, 0, x, y, midR)
    const mA = (0.06 + potentialNorm * 0.12) * da
    midG.addColorStop(0, rgba(COLORS.core, mA * 0.7))
    midG.addColorStop(0.3, rgba(COLORS.bright, mA * 0.4))
    midG.addColorStop(0.7, rgba(COLORS.mid, mA * 0.12))
    midG.addColorStop(1, 'rgba(0,0,0,0)')
    ctx.beginPath()
    ctx.arc(x, y, midR, 0, Math.PI * 2)
    ctx.fillStyle = midG
    ctx.fill()

    // Inner glow
    const innerR = r * (2.5 + potentialNorm * 1.5)
    const innerG = ctx.createRadialGradient(x, y, 0, x, y, innerR)
    const iA = (0.12 + potentialNorm * 0.3) * da
    innerG.addColorStop(0, rgba(COLORS.core, iA))
    innerG.addColorStop(0.4, rgba(COLORS.bright, iA * 0.5))
    innerG.addColorStop(1, 'rgba(0,0,0,0)')
    ctx.beginPath()
    ctx.arc(x, y, innerR, 0, Math.PI * 2)
    ctx.fillStyle = innerG
    ctx.fill()

    // Firing flash
    if (this.isFiring) {
      const flashR = r * (14 + potentialNorm * 10)
      const fG = ctx.createRadialGradient(x, y, 0, x, y, flashR)
      fG.addColorStop(0, rgba(COLORS.core, 0.25 * potentialNorm * da))
      fG.addColorStop(0.15, rgba(COLORS.bright, 0.12 * potentialNorm * da))
      fG.addColorStop(0.5, rgba(COLORS.mid, 0.04 * potentialNorm * da))
      fG.addColorStop(1, 'rgba(0,0,0,0)')
      ctx.beginPath()
      ctx.arc(x, y, flashR, 0, Math.PI * 2)
      ctx.fillStyle = fG
      ctx.fill()
    }

    ctx.restore()

    // Soma body
    const offX = -r * 0.25
    const offY = -r * 0.25

    const somaG = ctx.createRadialGradient(x + offX, y + offY, r * 0.1, x, y, r)
    somaG.addColorStop(0, rgba(COLORS.core, (0.3 + potentialNorm * 0.4) * da))
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
    if (this.depth > 0.4) {
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
    nucG.addColorStop(0, rgba(COLORS.core, (0.55 + potentialNorm * 0.4) * da))
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

  draw(ctx: CanvasRenderingContext2D) {
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
    ctx.save()
    ctx.globalCompositeOperation = 'lighter'
    const g = ctx.createRadialGradient(x, y, 0, x, y, 10)
    g.addColorStop(0, rgba(COLORS.core, 0.35))
    g.addColorStop(0.3, rgba(COLORS.bright, 0.12))
    g.addColorStop(1, 'rgba(0,0,0,0)')
    ctx.beginPath()
    ctx.arc(x, y, 10, 0, Math.PI * 2)
    ctx.fillStyle = g
    ctx.fill()
    ctx.restore()

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

  update(time: number, width: number, height: number) {
    const d = this.drift.get(time)
    this.x += d.x * 0.2
    this.y += d.y * 0.2
    if (this.x < -20) this.x = width + 20
    if (this.x > width + 20) this.x = -20
    if (this.y < -20) this.y = height + 20
    if (this.y > height + 20) this.y = -20
  }

  draw(ctx: CanvasRenderingContext2D, time: number) {
    const pulse = Math.sin(time * this.pulseSpeed + this.pulseOffset) * 0.2 + 0.8
    const alpha = this.brightness * pulse
    const sz = this.baseSize * (0.5 + this.z * 0.5) * pulse

    if (this.brightness > 0.2) {
      ctx.save()
      ctx.globalCompositeOperation = 'lighter'
      const glowSz = sz * 6
      const g = ctx.createRadialGradient(this.x, this.y, 0, this.x, this.y, glowSz)
      g.addColorStop(0, rgba(COLORS.bright, alpha * 0.12))
      g.addColorStop(0.5, rgba(COLORS.mid, alpha * 0.04))
      g.addColorStop(1, 'rgba(0,0,0,0)')
      ctx.beginPath()
      ctx.arc(this.x, this.y, glowSz, 0, Math.PI * 2)
      ctx.fillStyle = g
      ctx.fill()
      ctx.restore()
    }

    ctx.beginPath()
    ctx.arc(this.x, this.y, sz, 0, Math.PI * 2)
    ctx.fillStyle = rgba(COLORS.core, alpha * 0.7)
    ctx.fill()
  }
}

// ---------------------------------------------------------------------------
// Batched connection renderer
// ---------------------------------------------------------------------------

function drawConnections(
  ctx: CanvasRenderingContext2D,
  conns: SynapticConnection[],
  neuralIntensity: number,
  neuronCount: number,
  frameCount: number,
) {
  const buckets: SynapticConnection[][] = [[], [], []]
  // Connection LOD: skip every other frame when neuronCount > 60
  const skipOdd = neuronCount > 60 && frameCount % 2 === 0

  for (let i = 0; i < conns.length; i++) {
    if (skipOdd && i % 2 === 1) continue
    const c = conns[i]
    const dx = c.dendriteTip.x - c.preTerminal.x
    const dy = c.dendriteTip.y - c.preTerminal.y
    const dist = Math.sqrt(dx * dx + dy * dy)
    if (dist > 280) continue

    const baseAlpha = (1 - dist / 280) * 0.18 * c.strength
    const alpha = baseAlpha + neuralIntensity * baseAlpha * 1.5

    if (alpha > 0.12) buckets[0].push(c)
    else if (alpha > 0.06) buckets[1].push(c)
    else buckets[2].push(c)
  }

  const alphas = [0.12, 0.06, 0.03]
  const widths = [0.6, 0.4, 0.3]
  const glowWidths = [3, 2, 1.5]
  const boost = 1 + neuralIntensity * 2

  // Glow pass (additive)
  ctx.save()
  ctx.globalCompositeOperation = 'lighter'
  for (let b = 0; b < 3; b++) {
    if (buckets[b].length === 0) continue
    ctx.strokeStyle = rgba(COLORS.dim, Math.min(alphas[b] * boost * 0.5, 0.15))
    ctx.lineWidth = glowWidths[b]
    ctx.beginPath()
    for (let i = 0; i < buckets[b].length; i++) {
      const c = buckets[b][i]
      ctx.moveTo(c.preTerminal.x, c.preTerminal.y)
      ctx.lineTo(c.dendriteTip.x, c.dendriteTip.y)
    }
    ctx.stroke()
  }
  ctx.restore()

  // Crisp fiber pass
  for (let b = 0; b < 3; b++) {
    if (buckets[b].length === 0) continue
    ctx.strokeStyle = rgba(COLORS.mid, Math.min(alphas[b] * boost, 0.25))
    ctx.lineWidth = widths[b]
    ctx.beginPath()
    for (let i = 0; i < buckets[b].length; i++) {
      const c = buckets[b][i]
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
  signals: ActionPotential[]
  particles: BackgroundParticle[]
  intensity: number
  targetIntensity: number
  frameId: number
  lastTime: number
  frameCount: number
  width: number
  height: number
}

function createAnimState(width: number, height: number): AnimState {
  // Adaptive neuron count based on hardware
  const cores = typeof navigator !== 'undefined' ? (navigator.hardwareConcurrency ?? 4) : 4
  const neuronCount = Math.min(80, cores * 6)
  const particleCount = 250

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
          const dist = Math.sqrt(dx * dx + dy * dy)

          if (dist < 280 && Math.random() > 0.45) {
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

  return {
    neurons,
    connections,
    signals: [],
    particles,
    intensity: 0,
    targetIntensity: 0,
    frameId: 0,
    lastTime: 0,
    frameCount: 0,
    width,
    height,
  }
}

// ---------------------------------------------------------------------------
// React component
// ---------------------------------------------------------------------------

const NeuralCanvas = forwardRef<NeuralCanvasHandle>(function NeuralCanvas(_props, ref) {
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

    // Size to viewport
    const resize = () => {
      canvas.width = window.innerWidth
      canvas.height = window.innerHeight
      if (stateRef.current) {
        stateRef.current.width = canvas.width
        stateRef.current.height = canvas.height
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
    const state = createAnimState(canvas.width, canvas.height)
    stateRef.current = state

    // Animation loop
    const animate = (currentTime: number) => {
      const dt = Math.min(currentTime - state.lastTime, 50)
      state.lastTime = currentTime
      state.frameCount++

      // Smooth intensity lerp (fast ramp up, slow decay)
      const lerpRate = state.targetIntensity > state.intensity ? 0.12 : 0.02
      state.intensity += (state.targetIntensity - state.intensity) * lerpRate

      const w = canvas.width
      const h = canvas.height

      // Motion blur: semi-transparent fill instead of clearRect
      ctx.fillStyle = 'rgba(3,7,18,0.08)'
      ctx.fillRect(0, 0, w, h)

      // Ambient glow
      const cx = w / 2
      const cy = h / 2
      const ambR = Math.max(w, h) * 0.5
      const ambG = ctx.createRadialGradient(cx, cy, 0, cx, cy, ambR)
      ambG.addColorStop(0, rgba(COLORS.dim, 0.03 + state.intensity * 0.02))
      ambG.addColorStop(1, 'rgba(0,0,0,0)')
      ctx.fillStyle = ambG
      ctx.fillRect(0, 0, w, h)

      canvas.style.opacity = String(0.9 + state.intensity * 0.1)

      // Background particles (z < 0.5)
      state.particles.forEach((p) => {
        p.update(currentTime, w, h)
        if (p.z < 0.5) p.draw(ctx, currentTime)
      })

      // Connection fibers
      drawConnections(
        ctx,
        state.connections,
        state.intensity,
        state.neurons.length,
        state.frameCount,
      )

      // Update and draw neurons (sorted back-to-front)
      state.neurons.forEach((neuron) => {
        neuron.update(currentTime, dt, w, h)
        neuron.draw(ctx, currentTime)

        // Fire action potentials
        if (neuron.isFiring && neuron.firePhase === 1 && neuron.fireTimer < dt * 2) {
          const outgoing = neuron.outgoingConnections
          const maxSignals = 2 + Math.floor(state.intensity * 5)
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
          if (Math.random() < state.intensity * 0.025) {
            neuron.epsp += 25
          }
        }
      })

      // Action potentials
      for (let i = state.signals.length - 1; i >= 0; i--) {
        state.signals[i].update(dt)
        state.signals[i].draw(ctx)
        if (!state.signals[i].active) state.signals.splice(i, 1)
      }
      while (state.signals.length > 60) state.signals.shift()

      // Foreground particles (z >= 0.5)
      state.particles.forEach((p) => {
        if (p.z >= 0.5) p.draw(ctx, currentTime)
      })

      state.frameId = requestAnimationFrame(animate)
    }

    state.frameId = requestAnimationFrame(animate)

    // Cleanup
    return () => {
      cancelAnimationFrame(state.frameId)
      window.removeEventListener('resize', onResize)
      clearTimeout(resizeTimeout)
      stateRef.current = null
    }
  }, [])

  return (
    <canvas ref={canvasRef} className="fixed inset-0 z-0" style={{ willChange: 'transform' }} />
  )
})

NeuralCanvas.displayName = 'NeuralCanvas'

export default NeuralCanvas
