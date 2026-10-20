import type { ClubTactics } from '../bindings/ClubTactics'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import type { Spot } from '../selection'
import { conditionColour, fullName, outOfPosition, shortRole, surname, type Run } from '../tactics'

// Componentes da tela de Tácticas do CM, usados na tela de Tácticas e nas Tácticas a meio
// do jogo (as funções e o hook da seta estão em ../tactics.ts).

export type Bind = { 'data-spot': string; onMouseDown: (e: React.MouseEvent) => void }

export function Row({
  p,
  kind,
  place,
  bind,
  onToggle,
  onOpen,
}: {
  p: TacticPlayer
  kind: 'starter' | 'bench' | 'reserve'
  /** Função (titular) ou SB1–SB7 (suplente). */
  place: string
  bind: Bind
  onToggle?: () => void
  /** Duplo clique (abre o perfil). */
  onOpen?: () => void
}) {
  const text = kind === 'bench' ? 'text-[#8a5ad8]' : kind === 'reserve' ? 'text-[#8a8a8a]' : 'text-white'
  const toggle =
    onToggle &&
    ((e: React.MouseEvent) => {
      e.stopPropagation()
      onToggle()
    })
  return (
    <div {...bind} onDoubleClick={onOpen} className="flex h-[19px] cursor-grab items-center gap-2 px-1 text-[13.5px]">
      {/* Número e casa: clicar põe/tira da equipa */}
      <span
        onMouseDown={(e) => onToggle && e.stopPropagation()}
        onClick={toggle}
        title={onToggle ? 'Clique para pôr/tirar da equipa' : undefined}
        className={`w-7 shrink-0 text-center tabular-nums cm-cell ${onToggle ? 'cursor-pointer' : ''} ${kind === 'bench' ? '!bg-[#1d2d8a] text-white' : 'text-white'}`}
      >
        {p.squadNumber ?? ''}
      </span>
      <span
        onMouseDown={(e) => onToggle && e.stopPropagation()}
        onClick={toggle}
        className={`w-9 shrink-0 text-center text-[11px] ${onToggle ? 'cursor-pointer' : ''} ${
          kind === 'starter' && outOfPosition(p) ? 'cm-cell !bg-[#7a3d00] text-cm-orange' : 'cm-cell text-cm-yellow'
        }`}
        title={kind === 'starter' && p.fit !== null ? `Nota na função: ${p.fit}` : undefined}
      >
        {place}
      </span>
      <span className={`flex-1 truncate ${text}`}>{fullName(p.listName)}</span>
      {(p.injured || p.suspended) && (
        <span className="shrink-0 bg-[#9c0000] px-1 text-[11px] font-bold text-white">{p.injured ? 'Les' : 'Sus'}</span>
      )}
      <span className={`w-24 shrink-0 truncate ${text}`}>{p.position}</span>
      <span className={`w-10 shrink-0 text-right ${conditionColour(p.condition)}`}>{p.condition ?? 100}%</span>
    </div>
  )
}

/** Seta tracejada (em % do campo), como as setas de movimento do CM. */
function Arrow({ from, to, live }: { from: [number, number]; to: [number, number]; live?: boolean }) {
  // Ponta da seta: dois traços curtos para trás a partir do alvo
  const [x1, y1] = from
  const [x2, y2] = to
  const a = Math.atan2(y2 - y1, x2 - x1)
  const head = (d: number) => [x2 - 3 * Math.cos(a + d), y2 - 3 * Math.sin(a + d)]
  const [hx1, hy1] = head(0.5)
  const [hx2, hy2] = head(-0.5)
  const colour = live ? '#ffff40' : '#ffffff'
  return (
    <g stroke={colour} strokeWidth={1.5} fill="none" vectorEffect="non-scaling-stroke">
      <line x1={x1} y1={y1} x2={x2} y2={y2} strokeDasharray="4 3" vectorEffect="non-scaling-stroke" />
      <polyline points={`${hx1},${hy1} ${x2},${y2} ${hx2},${hy2}`} vectorEffect="non-scaling-stroke" />
    </g>
  )
}

/** Campo com a formação, como a "Vista Geral" do CM. Botão direito e arrastar a partir de
 * um jogador desenha a seta de movimento (para cima ataca, para baixo recua). */
export function Pitch({
  t,
  bind,
  run,
  onRunStart,
}: {
  t: ClubTactics
  bind: (s: Spot, label: string) => Bind
  /** Seta a ser desenhada agora (botão direito em curso). */
  run: Run | null
  onRunStart: (index: number) => void
}) {
  return (
    <div
      data-pitch
      onContextMenu={(e) => e.preventDefault()}
      className="relative min-h-0 flex-1 overflow-hidden border border-[#1d5a1d] bg-[repeating-linear-gradient(0deg,#1f6b22_0_36px,#1c6420_36px_72px)]"
    >
      {/* Linhas do campo */}
      <div className="pointer-events-none absolute inset-2 border border-white/40" />
      <div className="pointer-events-none absolute left-2 right-2 top-1/2 border-t border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-1/2 h-24 w-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-2 h-[16%] w-[44%] -translate-x-1/2 border border-t-0 border-white/40" />
      <div className="pointer-events-none absolute bottom-2 left-1/2 h-[16%] w-[44%] -translate-x-1/2 border border-b-0 border-white/40" />
      <svg className="pointer-events-none absolute inset-0 h-full w-full" viewBox="0 0 100 100" preserveAspectRatio="none">
        {t.starters.map((s, i) => (s.run && run?.index !== i ? <Arrow key={i} from={[s.x, s.y]} to={[s.run[0], s.run[1]]} /> : null))}
        {run && <Arrow from={[t.starters[run.index].x, t.starters[run.index].y]} to={[run.x, run.y]} live />}
      </svg>
      {t.starters.map((s, i) => {
        const b = bind({ kind: 'starter', index: i }, s.player ? surname(s.player.listName) : s.role)
        return (
          <div
            key={i}
            {...b}
            onMouseDown={(e) => {
              if (e.button === 2 && t.editable && s.role !== 'GR') {
                e.preventDefault()
                onRunStart(i)
              } else b.onMouseDown(e)
            }}
            className={`absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center ${t.editable ? 'cursor-grab' : ''}`}
            style={{ left: `${s.x}%`, top: `${s.y}%` }}
            title={s.role}
          >
            <span
              className={`flex h-6 w-6 items-center justify-center rounded-full border border-black text-[12px] font-bold ${
                s.role === 'GR' ? 'bg-[#0f9a2a] text-white' : s.player ? 'bg-white text-black' : 'bg-transparent text-white'
              }`}
            >
              {s.player?.squadNumber ?? ''}
            </span>
            <span
              className={`whitespace-nowrap text-[13px] font-bold [text-shadow:1px_1px_0_#000] ${
                outOfPosition(s.player) ? 'text-cm-orange' : s.player ? 'text-cm-yellow' : 'text-white/70'
              }`}
            >
              {s.player ? surname(s.player.listName) : shortRole(s.role)}
            </span>
          </div>
        )
      })}
    </div>
  )
}
