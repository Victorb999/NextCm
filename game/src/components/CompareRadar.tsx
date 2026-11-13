import type { PersonProfile } from '../bindings/PersonProfile'

// A aba Visão Geral do Comparar Jogadores (como no FM08): um radar com 8 grupos de atributos, a média
// de cada grupo (1–20) para os dois. Só a média dos atributos que o clube conhece; sem nenhum, o
// eixo fica no centro e o valor a "-".

type Axis = [string, string[]]

const FIELD: Axis[] = [
  ['Defesa', ['tackling', 'marking', 'positioning']],
  ['Físico', ['strength', 'stamina', 'balance', 'agility']],
  ['Velocidade', ['pace', 'acceleration']],
  ['Criatividade', ['vision', 'passing', 'flair']],
  ['Ataque', ['finishing', 'off_the_ball', 'long_shots']],
  ['Técnica', ['technique', 'dribbling', 'crossing']],
  ['Jogo Aéreo', ['heading', 'jumping']],
  ['Mental', ['decisions', 'anticipation', 'determination', 'teamwork', 'work_rate', 'bravery']],
]

const KEEPER: Axis[] = [
  ['Jogo Mãos', ['handling']],
  ['Reflexos', ['reflexes']],
  ['Posicionamento', ['positioning', 'anticipation']],
  ['Jogo Aéreo', ['jumping', 'bravery']],
  ['Passe', ['passing', 'vision']],
  ['Físico', ['strength', 'agility', 'balance']],
  ['Velocidade', ['pace', 'acceleration']],
  ['Mental', ['decisions', 'determination', 'teamwork', 'leadership']],
]

const isKeeper = (p: PersonProfile) => p.player?.position.startsWith('GR') ?? false

function average(p: PersonProfile, keys: string[]): number | null {
  const known = keys.map((k) => p.player?.attributes[k]).filter((v): v is number => v != null)
  return known.length ? known.reduce((s, v) => s + v, 0) / known.length : null
}

// Largo o bastante para os rótulos dos lados ("Jogo Aéreo", "Velocidade") caberem
const W = 520
const H = 400
const CX = W / 2
const CY = H / 2
const R = 145

function point(i: number, n: number, v: number): [number, number] {
  const angle = -Math.PI / 2 + (2 * Math.PI * i) / n
  const r = (R * Math.max(0, Math.min(20, v))) / 20
  return [CX + r * Math.cos(angle), CY + r * Math.sin(angle)]
}

const COLOURS = ['rgb(var(--c-yellow))', 'rgb(var(--c-purplelight))']

/** O radar dos dois jogadores (os eixos dos guarda-redes se os dois o forem). */
export default function CompareRadar({ people }: { people: [PersonProfile, PersonProfile] }) {
  const axes = people.every(isKeeper) ? KEEPER : FIELD
  const n = axes.length
  const values = people.map((p) => axes.map(([, keys]) => average(p, keys)))
  return (
    <div className="flex items-center justify-center gap-4 p-2">
      <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} className="shrink-0">
        {[5, 10, 15, 20].map((ring) => (
          <polygon
            key={ring}
            points={axes.map((_, i) => point(i, n, ring).join(',')).join(' ')}
            fill="none"
            stroke="rgb(var(--c-line))"
            strokeWidth={ring === 20 ? 1.5 : 1}
          />
        ))}
        {axes.map(([label], i) => {
          const [x, y] = point(i, n, 20)
          const [lx, ly] = point(i, n, 23.5)
          const anchor = Math.abs(lx - CX) < 4 ? 'middle' : lx > CX ? 'start' : 'end'
          return (
            <g key={label}>
              <line x1={CX} y1={CY} x2={x} y2={y} stroke="rgb(var(--c-line))" />
              <text x={lx} y={ly + 5} textAnchor={anchor} fill="white" fontSize={14}>
                {label}
              </text>
            </g>
          )
        })}
        {values.map((vs, j) => (
          <polygon
            key={j}
            points={vs.map((v, i) => point(i, n, v ?? 0).join(',')).join(' ')}
            fill={COLOURS[j]}
            fillOpacity={0.18}
            stroke={COLOURS[j]}
            strokeWidth={2}
          />
        ))}
      </svg>
      <div className="w-80 text-[14px] tabular-nums">
        <div className="grid h-6 grid-cols-[1fr_5rem_5rem] items-center gap-x-2 border-b border-cm-line px-2 font-title">
          <span className="text-white">Média (1–20)</span>
          {people.map((p, j) => (
            <span key={p.id} className="truncate text-right font-bold" style={{ color: COLOURS[j] }} title={p.name}>
              {p.name.split(' ').at(-1)}
            </span>
          ))}
        </div>
        <div className="cm-zebra">
          {axes.map(([label], i) => {
            const [x, y] = [values[0]?.[i] ?? null, values[1]?.[i] ?? null]
            const cell = (v: number | null, o: number | null) => (
              <span
                className={`text-right ${v == null ? 'text-[#777]' : o != null && v > o + 0.05 ? 'font-bold text-cm-green' : 'text-cm-orange'}`}
              >
                {v == null ? '-' : v.toFixed(1)}
              </span>
            )
            return (
              <div key={label} className="grid h-6 grid-cols-[1fr_5rem_5rem] items-center gap-x-2 px-2">
                <span className="text-white">{label}</span>
                {cell(x, y)}
                {cell(y, x)}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
