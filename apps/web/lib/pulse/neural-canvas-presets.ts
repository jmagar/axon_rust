export interface NeuralCanvasRGB {
  r: number
  g: number
  b: number
}

export type NeuralCanvasProfile = 'current' | 'subtle' | 'cinematic' | 'electric' | 'zen'

export interface NeuralCanvasPalette {
  core: NeuralCanvasRGB
  bright: NeuralCanvasRGB
  mid: NeuralCanvasRGB
  dim: NeuralCanvasRGB
  faint: NeuralCanvasRGB
  backgroundInner: NeuralCanvasRGB
  backgroundOuter: NeuralCanvasRGB
}

export interface VisualPresetConfig {
  brightness: number
  density: number
  glow: number
  pulse: number
  activity: number
  backgroundInterval: number
  parallaxDepth: number
  burstThreshold: number
  burstStrength: number
  calmRecovery: number
  palette: NeuralCanvasPalette
}

const BASE_PALETTE: NeuralCanvasPalette = {
  core: { r: 236, g: 243, b: 255 },
  bright: { r: 142, g: 206, b: 255 },
  mid: { r: 102, g: 145, b: 232 },
  dim: { r: 123, g: 92, b: 176 },
  faint: { r: 46, g: 30, b: 84 },
  backgroundInner: { r: 30, g: 58, b: 164 },
  backgroundOuter: { r: 5, g: 8, b: 24 },
}

export const NEURAL_CANVAS_PRESETS: Record<NeuralCanvasProfile, VisualPresetConfig> = {
  // Canonical baseline: always available and used as default fallback.
  current: {
    brightness: 1.08,
    density: 1.08,
    glow: 1.1,
    pulse: 1,
    activity: 1,
    backgroundInterval: 2,
    parallaxDepth: 1,
    burstThreshold: 0.72,
    burstStrength: 1,
    calmRecovery: 1,
    palette: BASE_PALETTE,
  },
  subtle: {
    brightness: 0.9,
    density: 0.9,
    glow: 0.9,
    pulse: 0.75,
    activity: 0.85,
    backgroundInterval: 3,
    parallaxDepth: 0.82,
    burstThreshold: 0.82,
    burstStrength: 0.72,
    calmRecovery: 1.2,
    palette: {
      ...BASE_PALETTE,
      backgroundInner: { r: 8, g: 38, b: 116 },
      backgroundOuter: { r: 1, g: 6, b: 22 },
    },
  },
  cinematic: {
    brightness: 1.15,
    density: 1.12,
    glow: 1.16,
    pulse: 1.05,
    activity: 1.05,
    backgroundInterval: 2,
    parallaxDepth: 1.08,
    burstThreshold: 0.68,
    burstStrength: 1.15,
    calmRecovery: 0.92,
    palette: {
      ...BASE_PALETTE,
      backgroundInner: { r: 14, g: 62, b: 178 },
      backgroundOuter: { r: 3, g: 12, b: 36 },
    },
  },
  electric: {
    brightness: 1.25,
    density: 1.2,
    glow: 1.28,
    pulse: 1.35,
    activity: 1.2,
    backgroundInterval: 1,
    parallaxDepth: 1.22,
    burstThreshold: 0.6,
    burstStrength: 1.35,
    calmRecovery: 0.82,
    palette: {
      ...BASE_PALETTE,
      bright: { r: 92, g: 205, b: 255 },
      backgroundInner: { r: 18, g: 86, b: 226 },
      backgroundOuter: { r: 4, g: 16, b: 44 },
    },
  },
  zen: {
    brightness: 0.3,
    density: 0.4,
    glow: 0.2,
    pulse: 0.25,
    activity: 0.2,
    backgroundInterval: 6,
    parallaxDepth: 0.3,
    burstThreshold: 0.95,
    burstStrength: 0.15,
    calmRecovery: 2.0,
    palette: {
      core: { r: 180, g: 210, b: 255 },
      bright: { r: 30, g: 100, b: 180 },
      mid: { r: 10, g: 55, b: 130 },
      dim: { r: 5, g: 28, b: 80 },
      faint: { r: 2, g: 12, b: 40 },
      backgroundInner: { r: 4, g: 18, b: 58 },
      backgroundOuter: { r: 1, g: 3, b: 12 },
    },
  },
}

export const DEFAULT_NEURAL_CANVAS_PROFILE: NeuralCanvasProfile = 'current'

export function getNeuralCanvasPreset(profile?: NeuralCanvasProfile): VisualPresetConfig {
  return NEURAL_CANVAS_PRESETS[profile ?? DEFAULT_NEURAL_CANVAS_PROFILE]
}
