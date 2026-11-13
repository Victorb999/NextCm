// Sons da partida, sintetizados com Web Audio (sem ficheiros): a torcida de fundo, o lance
// perigoso, o golo e o apito. Plano em plans/006-sons-da-partida.md.

import { getSettings } from './settings'

let ctx: AudioContext | null = null
let master: GainNode | null = null
let noise: AudioBuffer | null = null
let crowd: { source: AudioBufferSourceNode; lfo: OscillatorNode; gain: GainNode } | null = null

/** Volume da torcida parada e no pico de um lance perigoso. */
const CROWD_BASE = 0.1
const CROWD_SURGE = 0.22

const enabled = () => getSettings().sound

/** O contexto de áudio, criado na primeira vez (o browser só o deixa tocar depois de um clique). */
function audio(): AudioContext | null {
  if (typeof AudioContext === 'undefined') return null
  if (!ctx) {
    ctx = new AudioContext()
    master = ctx.createGain()
    master.connect(ctx.destination)
    noise = pinkNoise(ctx, 2)
  }
  if (ctx.state === 'suspended') void ctx.resume()
  master!.gain.value = getSettings().volume
  return ctx
}

// O primeiro clique destrava o áudio (antes disso o AudioContext fica suspenso)
if (typeof window !== 'undefined') {
  window.addEventListener('pointerdown', () => void ctx?.resume(), { capture: true })
}

/** Ruído rosa (Paul Kellet), `seconds` de comprimento, para tocar em loop. */
function pinkNoise(c: AudioContext, seconds: number): AudioBuffer {
  const buf = c.createBuffer(1, c.sampleRate * seconds, c.sampleRate)
  const d = buf.getChannelData(0)
  let b0 = 0,
    b1 = 0,
    b2 = 0,
    b3 = 0,
    b4 = 0,
    b5 = 0,
    b6 = 0
  for (let i = 0; i < d.length; i++) {
    const w = Math.random() * 2 - 1
    b0 = 0.99886 * b0 + w * 0.0555179
    b1 = 0.99332 * b1 + w * 0.0750759
    b2 = 0.969 * b2 + w * 0.153852
    b3 = 0.8665 * b3 + w * 0.3104856
    b4 = 0.55 * b4 + w * 0.5329522
    b5 = -0.7616 * b5 - w * 0.016898
    d[i] = (b0 + b1 + b2 + b3 + b4 + b5 + b6 + w * 0.5362) * 0.11
    b6 = w * 0.115926
  }
  return buf
}

/** Uma fonte de ruído em loop a começar num ponto ao acaso (duas fontes não soam iguais). */
function noiseSource(c: AudioContext): AudioBufferSourceNode {
  const s = c.createBufferSource()
  s.buffer = noise
  s.loop = true
  s.loopStart = 0
  s.loopEnd = noise!.duration
  return s
}

function filter(c: AudioContext, type: BiquadFilterType, frequency: number, q = 0.7): BiquadFilterNode {
  const f = c.createBiquadFilter()
  f.type = type
  f.frequency.value = frequency
  f.Q.value = q
  return f
}

/** Liga os nós em fila e devolve o último. */
function chain(first: AudioNode, ...rest: AudioNode[]): AudioNode {
  let last = first
  for (const n of rest) {
    last.connect(n)
    last = n
  }
  return last
}

/** A torcida de fundo: liga com o jogo a correr. */
export function crowdStart() {
  if (!enabled() || crowd) return
  const c = audio()
  if (!c) return
  const source = noiseSource(c)
  const gain = c.createGain()
  gain.gain.setValueAtTime(0, c.currentTime)
  gain.gain.linearRampToValueAtTime(CROWD_BASE, c.currentTime + 0.8)
  // Ondula devagar, como uma bancada
  const lfo = c.createOscillator()
  lfo.frequency.value = 0.13
  const depth = c.createGain()
  depth.gain.value = CROWD_BASE * 0.25
  lfo.connect(depth).connect(gain.gain)
  chain(source, filter(c, 'bandpass', 500, 0.6), filter(c, 'lowpass', 2200), gain, master!)
  source.start(0, Math.random() * noise!.duration)
  lfo.start()
  crowd = { source, lfo, gain }
}

/** Pára a torcida (intervalo, fim, sair da partida). */
export function crowdStop() {
  if (!crowd || !ctx) return
  const { source, lfo, gain } = crowd
  const t = ctx.currentTime
  gain.gain.cancelScheduledValues(t)
  gain.gain.setValueAtTime(gain.gain.value, t)
  gain.gain.linearRampToValueAtTime(0, t + 0.6)
  source.stop(t + 0.7)
  lfo.stop(t + 0.7)
  crowd = null
}

/** Lance perigoso: a torcida sobe e volta. */
export function crowdSurge() {
  if (!crowd || !ctx) return
  const g = crowd.gain.gain
  const t = ctx.currentTime
  g.cancelScheduledValues(t)
  g.setValueAtTime(g.value, t)
  g.linearRampToValueAtTime(CROWD_SURGE, t + 0.4)
  g.linearRampToValueAtTime(CROWD_BASE, t + 1.9)
}

/** Uma explosão de ruído filtrado com ataque e queda (o rugido, o "ohh"). */
function burst(c: AudioContext, filters: BiquadFilterNode[], peak: number, attack: number, length: number) {
  const s = noiseSource(c)
  const g = c.createGain()
  const t = c.currentTime
  g.gain.setValueAtTime(0.0001, t)
  g.gain.linearRampToValueAtTime(peak, t + attack)
  g.gain.exponentialRampToValueAtTime(0.0001, t + length)
  chain(s, ...filters, g, master!)
  s.start(t, Math.random() * noise!.duration)
  s.stop(t + length + 0.1)
}

/** Golo: o rugido da casa ou o "ohh" quando marca o visitante. */
export function goal(home: boolean) {
  if (!enabled()) return
  const c = audio()
  if (!c) return
  if (home) {
    burst(c, [filter(c, 'highpass', 300), filter(c, 'lowpass', 3000)], 0.55, 0.15, 3.5)
  } else {
    burst(c, [filter(c, 'lowpass', 600)], 0.25, 0.1, 1.2)
    burst(c, [filter(c, 'highpass', 300), filter(c, 'lowpass', 2500)], 0.18, 0.2, 2.5)
  }
}

/** O apito do árbitro: `blasts` em segundos (início e intervalo, dois curtos; fim, curto, curto, longo). */
function whistle(blasts: [number, number][]) {
  if (!enabled()) return
  const c = audio()
  if (!c) return
  const t0 = c.currentTime + 0.05
  for (const [at, length] of blasts) {
    const t = t0 + at
    const g = c.createGain()
    g.gain.setValueAtTime(0.0001, t)
    g.gain.linearRampToValueAtTime(0.06, t + 0.02)
    g.gain.setValueAtTime(0.06, t + length - 0.03)
    g.gain.linearRampToValueAtTime(0.0001, t + length)
    g.connect(master!)
    // A ervilha do apito: duas notas perto e um trilo rápido
    for (const f of [2900, 3100]) {
      const o = c.createOscillator()
      o.frequency.value = f
      const trill = c.createOscillator()
      trill.frequency.value = 30
      const depth = c.createGain()
      depth.gain.value = 80
      trill.connect(depth).connect(o.frequency)
      o.connect(g)
      o.start(t)
      trill.start(t)
      o.stop(t + length + 0.05)
      trill.stop(t + length + 0.05)
    }
  }
}

export const whistleShort = () =>
  whistle([
    [0, 0.25],
    [0.35, 0.25],
  ])
export const whistleEnd = () =>
  whistle([
    [0, 0.25],
    [0.35, 0.25],
    [0.7, 0.8],
  ])
