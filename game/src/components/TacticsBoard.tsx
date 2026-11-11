import type { ReactNode } from 'react'
import PitchField from './PitchField'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import type { Spot } from '../selection'
import { MENTALITY, PASSING, TACKLING, conditionColour, fullName, outOfPosition, shortRole, surname, type Run } from '../tactics'
import { PanelTitle } from './Screen'

// Componentes da tela de Tácticas do CM, usados na tela de Tácticas e nas Tácticas a meio
// do jogo (as funções e o hook da seta estão em ../tactics.ts).

export type Bind = { 'data-spot': string; onMouseDown: (e: React.MouseEvent) => void }

/** Cor da casa da função, pela linha do campo (como no Stitch): GR, defesa, meio, ataque. */
function roleChip(place: string) {
  if (place.startsWith('SB')) return 'bg-[#2a1450] text-[#d0bcff]'
  if (place.startsWith('GR')) return 'bg-[#12323a] text-[#5ad1e6]'
  if (place.startsWith('D') || place.startsWith('AL')) return 'bg-[#16244a] text-[#7aa2ff]'
  if (place.startsWith('M')) return 'bg-[#3a2508] text-cm-orange'
  return 'bg-[#3d0f1c] text-[#ff6b8a]'
}

// Larguras das colunas, iguais no cabeçalho e nas linhas
const COL = {
  num: 'w-8 shrink-0 text-center',
  place: 'w-12 shrink-0',
  pos: 'w-28 shrink-0 truncate',
  cond: 'w-12 shrink-0 text-right',
  rating: 'w-10 shrink-0 text-right',
  stat: 'w-12 shrink-0 text-center',
}

/** Cabeçalho das colunas da lista (onze, banco, resto do plantel). */
export function RowHeader({ place, action, rating, column = 'Posição' }: { place: string; action?: string; rating?: boolean; column?: string }) {
  return (
    <div className="flex h-7 items-center gap-2 bg-black/40 px-3 text-[11px] font-bold font-mono uppercase tracking-[0.12em] text-[#86869a]">
      <span className={COL.num}>Nº</span>
      <span className={COL.place}>{place}</span>
      <span className="flex-1">Jogador</span>
      <span className={COL.pos}>{column}</span>
      <span className={COL.cond}>Cond</span>
      {rating && <span className={COL.rating}>Nota</span>}
      <span className={COL.stat}>Est</span>
      {action !== undefined && <span className="w-[84px] shrink-0 text-right">{action}</span>}
    </div>
  )
}

export function Row({
  p,
  kind,
  place,
  bind,
  onToggle,
  onOpen,
  onOrders,
  ordered,
  captain,
  action,
  highlight,
  rating,
  note,
  column,
}: {
  p: TacticPlayer
  kind: 'starter' | 'bench' | 'reserve'
  /** Função (titular) ou SB1–SB9 (suplente). */
  place: string
  bind: Bind
  onToggle?: () => void
  /** Abre o perfil: um clique no nome (nas duas equipas) ou duplo clique na linha. */
  onOpen?: () => void
  /** Botão direito: instruções individuais. */
  onOrders?: () => void
  /** Tem instruções individuais diferentes das da equipa. */
  ordered?: boolean
  /** O capitão ("C" antes do nome). */
  captain?: boolean
  /** Botão no fim da linha (troca rápida no onze, Substituir no banco). */
  action?: ReactNode
  /** "wait": o jogador escolhido para a troca; "pick": pode ser o outro da troca. */
  highlight?: 'wait' | 'pick'
  /** A meio do jogo: a nota na partida (a coluna só aparece quando vem). */
  rating?: number | null
  /** Um aviso curto depois do nome ("sai no Ok"). */
  note?: ReactNode
  /** O Vêr: o valor no lugar da posição (`on` a amarelo, como o "Sim" do CM). */
  column?: { text: string; on: boolean }
}) {
  const text = kind === 'reserve' ? 'text-[#9a9aa6]' : 'text-white'
  const toggle =
    onToggle &&
    ((e: React.MouseEvent) => {
      e.stopPropagation()
      onToggle()
    })
  const out = kind === 'starter' && outOfPosition(p)
  return (
    <div
      {...bind}
      onDoubleClick={onOpen}
      onContextMenu={
        onOrders &&
        ((e) => {
          e.preventDefault()
          onOrders()
        })
      }
      className={`flex h-[30px] cursor-grab items-center gap-2 px-3 text-[14.5px] ${
        highlight === 'wait' ? '!bg-cm-purple/50' : highlight === 'pick' ? 'cursor-pointer hover:!bg-cm-yellow/15' : ''
      }`}
    >
      {/* Número e casa: clicar põe/tira da equipa */}
      <span
        onMouseDown={(e) => onToggle && e.stopPropagation()}
        onClick={toggle}
        title={onToggle ? 'Clique para pôr/tirar da equipa' : undefined}
        className={`${COL.num} font-semibold tabular-nums ${text} ${onToggle ? 'cursor-pointer' : ''}`}
      >
        {p.squadNumber ?? ''}
      </span>
      <span className={COL.place}>
        {place && (
          <span
            onMouseDown={(e) => onToggle && e.stopPropagation()}
            onClick={toggle}
            className={`inline-block min-w-[38px] rounded-[3px] px-1 text-center text-[11px] font-bold leading-[18px] tracking-wide ${
              out ? 'bg-[#7a3d00] text-cm-orange' : roleChip(place)
            } ${onToggle ? 'cursor-pointer' : ''}`}
            title={kind === 'starter' && p.fit !== null ? `Nota na função: ${p.fit}` : undefined}
          >
            {place}
          </span>
        )}
      </span>
      <span className={`flex min-w-0 flex-1 items-center gap-1.5 ${text}`}>
        {captain && <span className="shrink-0 rounded-[3px] bg-[#4a3300] px-1 text-[11px] font-bold leading-[16px] text-cm-yellow">C</span>}
        <span
          className={`truncate ${onOpen ? 'cursor-pointer hover:text-cm-yellow hover:underline' : ''}`}
          onMouseDown={(e) => onOpen && e.stopPropagation()}
          onClick={
            onOpen &&
            ((e) => {
              e.stopPropagation()
              onOpen()
            })
          }
          title={onOpen ? 'Ver o perfil' : undefined}
        >
          {fullName(p.listName)}
          {p.key && '*'}
        </span>
        {ordered && (
          <span
            className="shrink-0 rounded-[3px] bg-[#3a1840] px-1 text-[11px] font-bold leading-[16px] text-cm-magenta"
            title="Tem instruções individuais (as ordens próprias do jogador, no diálogo dele)"
          >
            Instr.
          </span>
        )}
        {note && <span className="shrink-0 text-[11px] font-bold text-[#ff6060]">{note}</span>}
      </span>
      {column ? (
        <span className={`${COL.pos} text-[13px] ${column.on ? 'font-bold text-cm-yellow' : 'text-[#9b6fe8]'}`}>{column.text}</span>
      ) : (
        <span className={`${COL.pos} text-[13px] text-[#c8c8d0]`}>{p.position}</span>
      )}
      <span className={`${COL.cond} font-semibold tabular-nums ${conditionColour(p.condition)}`}>{p.condition ?? 100}%</span>
      {rating !== undefined && <span className={`${COL.rating} font-semibold tabular-nums text-cm-yellow`}>{rating ?? '-'}</span>}
      <span className={COL.stat}>
        {p.injured || p.suspended ? (
          <span className="rounded-[3px] bg-[#9c0000] px-1.5 text-[11px] font-bold text-white">{p.injured ? 'Les' : 'Sus'}</span>
        ) : (
          <span className="rounded-[3px] bg-white/10 px-1.5 text-[11px] font-bold text-[#c8c8d0]">OK</span>
        )}
      </span>
      {action}
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
  colours,
}: {
  t: ClubTactics
  /** As cores do clube nos quadrados dos jogadores (o guarda-redes fica verde). */
  colours?: { fg: string; bg: string }
  bind: (s: Spot, label: string) => Bind
  /** Seta a ser desenhada agora (botão direito em curso). */
  run: Run | null
  onRunStart: (index: number) => void
}) {
  return (
    // O campo mantém a proporção de um campo a sério (68 x 105) e fica centrado no espaço que tiver
    <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden [container-type:size]">
      <PitchField
        data-pitch
        onContextMenu={(e) => e.preventDefault()}
        // O maior campo que cabe no espaço, sempre na mesma proporção
        style={{ width: 'min(100cqw, 100cqh * 0.7)' }}
      >
        <svg className="pointer-events-none absolute inset-0 h-full w-full" viewBox="0 0 100 100" preserveAspectRatio="none">
          {t.starters.map((s, i) => (s.run && run?.index !== i ? <Arrow key={i} from={[s.x, s.y]} to={[s.run[0], s.run[1]]} /> : null))}
          {run && <Arrow from={[t.starters[run.index].x, t.starters[run.index].y]} to={[run.x, run.y]} live />}
        </svg>
        {t.starters.map((s, i) => {
          const b = bind({ kind: 'starter', index: i }, s.player ? surname(s.player.listName) : s.role)
          return (
            <div
              // Pelo jogador: ao mudar de lugar desliza (o lugar vazio, pelo índice)
              key={s.player ? `p${s.player.id}` : `slot${i}`}
              {...b}
              onMouseDown={(e) => {
                if (e.button === 2 && t.editable && s.role !== 'GR') {
                  e.preventDefault()
                  onRunStart(i)
                } else b.onMouseDown(e)
              }}
              className={`cm-slide absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center gap-1 ${t.editable ? 'cursor-grab' : ''}`}
              style={{ left: `${s.x}%`, top: `${s.y}%` }}
              title={s.role}
            >
              <span
                className={`flex size-8 items-center justify-center rounded-lg text-[14px] font-bold shadow-[0_2px_6px_rgba(0,0,0,0.5)] ${
                  s.role === 'GR'
                    ? 'bg-[#4edea3] text-[#06281b]'
                    : s.player
                      ? colours
                        ? 'ring-1 ring-black/40'
                        : 'bg-white text-[#141414]'
                      : 'border-2 border-dashed border-white/50 text-white'
                }`}
                style={s.role !== 'GR' && s.player && colours ? { background: colours.bg, color: colours.fg } : undefined}
              >
                {s.player?.squadNumber ?? ''}
              </span>
              <span
                className={`whitespace-nowrap rounded bg-black/75 px-1.5 text-[12px] font-bold leading-[18px] ${
                  outOfPosition(s.player) ? 'text-cm-orange' : s.player ? 'text-white' : 'text-white/60'
                }`}
              >
                {s.player ? surname(s.player.listName) : shortRole(s.role)}
              </span>
            </div>
          )
        })}
      </PitchField>
    </div>
  )
}

/** Resumo das Instruções à Equipa ao lado do campo; Editar abre o diálogo de sempre. */
export function InstructionsSummary({ t, onEdit }: { t: ClubTactics; onEdit?: () => void }) {
  const i = t.instructions
  const name = (id: number | null) => {
    const p = t.starters.find((s) => s.player?.id === id)?.player
    return p ? surname(p.listName) : 'Adjunto'
  }
  const items: [string, string, boolean][] = [
    ['Mentalidade', MENTALITY.find(([k]) => k === i.mentality)?.[1] ?? '-', true],
    ['Passe', PASSING.find(([k]) => k === i.passing)?.[1] ?? '-', true],
    ['Desarme', TACKLING.find(([k]) => k === i.tackling)?.[1] ?? '-', true],
    ['Pressão', i.pressing ? 'Sim' : 'Não', i.pressing],
    ['Defesa em Linha', i.offsideTrap ? 'Sim' : 'Não', i.offsideTrap],
    ['Contra-Ataque', i.counterAttack ? 'Sim' : 'Não', i.counterAttack],
    ['Defesa Intensa', i.tightMarking ? 'Sim' : 'Não', i.tightMarking],
    ['Penáltis', name(t.takers.penalty), false],
    ['Livres', name(t.takers.freeKick), false],
  ]
  return (
    <div className="flex w-[176px] shrink-0 flex-col overflow-hidden rounded-md bg-cm-surface">
      <PanelTitle
        title="Instruções"
        right={
          onEdit && (
            <button onClick={onEdit} className="font-bold font-mono uppercase tracking-wider text-[#4edea3] hover:text-white">
              Editar
            </button>
          )
        }
      />
      <div className="flex min-h-0 flex-col gap-2 overflow-auto p-2.5">
        {items.map(([label, value, on]) => (
          <div key={label} className="min-w-0 rounded bg-[#1e1f25] px-2.5 py-1.5">
            <div className="truncate text-[10px] font-bold font-mono uppercase tracking-wider text-[#86869a]">{label}</div>
            <div className={`truncate font-semibold ${on ? 'text-[#4edea3]' : 'text-[#c8c8d0]'}`}>{value}</div>
          </div>
        ))}
      </div>
    </div>
  )
}
