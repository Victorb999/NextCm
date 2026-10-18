import { useEffect, useRef, useState } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { Selection } from '../bindings/Selection'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import InstructionsDialog from '../components/InstructionsDialog'
import { BottomBar, Link, PurpleMenu, Tabs, TitleBand } from '../components/Screen'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'
import { BENCH, selectionOf, swap, useDragSwap, type Spot } from '../selection'

// Formações na ordem do core (FORMATIONS em core/src/tactics.rs); a 0 ("não definida") não aparece
const FORMATIONS = ['4-4-2', '5-3-2 Líbero', '5-3-2', '4-4-2', '4-3-3', '4-2-4', '3-5-2', '4-1-2-1-2', '4-5-1', '3-4-3']
// Menu Tácticas, como no CM: Instruções à Equipa e as tácticas pré-definidas
const TACTICS_MENU = [
  { key: 'instructions', label: 'Instruções à Equipa' },
  ...FORMATIONS.map((label, i) => ({ key: String(i), label: `Pré-definida: ${label}` })).slice(1),
]

/** "Sobrenome, Nome" -> "Sobrenome" (como os nomes no campo do CM). */
const surname = (listName: string) => listName.split(', ')[0]

/** Nome na lista: "Nome Sobrenome", como na tela de tácticas do CM. */
const fullName = (listName: string) => {
  const [last, first] = listName.split(', ')
  return first ? `${first} ${last}` : last
}

/** "D D" -> "DD": a sigla curta da função. */
const shortRole = (role: string) => role.replace(' ', '')

/** Fora da posição: nota na função abaixo de 15 (mostrada em laranja). */
const outOfPosition = (p: TacticPlayer | null | undefined) => !!p && p.fit !== null && p.fit < 15

function conditionColour(c: number | null) {
  if (c === null || c >= 90) return 'text-cm-yellow'
  if (c >= 75) return 'text-cm-orange'
  return 'text-[#ff4040]'
}

type Bind = { 'data-spot': string; onMouseDown: (e: React.MouseEvent) => void }

function Row({
  p,
  kind,
  place,
  bind,
  onToggle,
}: {
  p: TacticPlayer
  kind: 'starter' | 'bench' | 'reserve'
  /** Função (titular) ou SB1–SB7 (suplente). */
  place: string
  bind: Bind
  onToggle?: () => void
}) {
  const nav = useNav()
  const text = kind === 'bench' ? 'text-[#8a5ad8]' : kind === 'reserve' ? 'text-[#8a8a8a]' : 'text-white'
  const toggle =
    onToggle &&
    ((e: React.MouseEvent) => {
      e.stopPropagation()
      onToggle()
    })
  return (
    <div
      {...bind}
      onDoubleClick={() => nav.go({ kind: 'person', id: p.id })}
      className="flex h-[19px] cursor-grab items-center gap-2 px-1 text-[13.5px]"
    >
      {/* Número e casa: clicar põe/tira da equipa */}
      <span
        onMouseDown={(e) => e.stopPropagation()}
        onClick={toggle}
        title={onToggle ? 'Clique para pôr/tirar da equipa' : undefined}
        className={`w-7 shrink-0 cursor-pointer text-center tabular-nums ${kind === 'bench' ? 'bg-[#1d2d8a] text-white' : 'text-white'}`}
      >
        {p.squadNumber ?? ''}
      </span>
      <span
        onMouseDown={(e) => e.stopPropagation()}
        onClick={toggle}
        className={`w-9 shrink-0 cursor-pointer text-center text-[11px] ${
          kind === 'starter' && outOfPosition(p) ? 'bg-[#7a3d00] text-cm-orange' : 'bg-[#262626] text-cm-yellow'
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

/** Campo com a formação, como a "Vista Geral" do CM. */
function Pitch({ t, bind }: { t: ClubTactics; bind: (s: Spot, label: string) => Bind }) {
  return (
    <div
      data-pitch
      className="relative min-h-0 flex-1 overflow-hidden border border-[#1d5a1d] bg-[repeating-linear-gradient(0deg,#1f6b22_0_36px,#1c6420_36px_72px)]"
    >
      {/* Linhas do campo */}
      <div className="pointer-events-none absolute inset-2 border border-white/40" />
      <div className="pointer-events-none absolute left-2 right-2 top-1/2 border-t border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-1/2 h-24 w-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-2 h-[16%] w-[44%] -translate-x-1/2 border border-t-0 border-white/40" />
      <div className="pointer-events-none absolute bottom-2 left-1/2 h-[16%] w-[44%] -translate-x-1/2 border border-b-0 border-white/40" />
      {t.starters.map((s, i) => (
        <div
          key={i}
          {...bind({ kind: 'starter', index: i }, s.player ? surname(s.player.listName) : s.role)}
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
      ))}
    </div>
  )
}

/**
 * Tela "Tácticas <clube>". No clube do treinador as mudanças valem na hora (arrastar troca,
 * clicar no número ou na casa põe/tira da equipa); Cancelar volta ao que estava ao abrir.
 */
export default function TacticsPage({ id }: { id: number }) {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [t, setT] = useState<ClubTactics | null>(null)
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [instructions, setInstructions] = useState(false)
  // A escolha ao abrir a tela, para o Cancelar
  const original = useRef<Selection | null>(null)
  const changed = useRef(false)

  useEffect(() => {
    api.getClubTactics(id).then(
      (tt) => {
        original.current = selectionOf(tt)
        setT(tt)
      },
      (e) => setError(String(e)),
    )
    api.getClub(id).then(setClub, () => {})
  }, [id])

  const colours = club?.club.fg && club?.club.bg ? { fg: club.club.fg, bg: club.club.bg } : { fg: '#ffffff', bg: '#3a3a3a' }
  const name = club?.club.shortName ?? t?.clubName ?? ''
  const editable = !!t?.editable

  /** Grava a escolha já (a táctica do jogo muda na hora) e mostra o resultado. */
  const apply = async (sel: Selection) => {
    try {
      setT(await api.setTactics(sel))
      changed.current = true
      setError(null)
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    }
  }

  const toggle = async (player: number) => {
    if (!t) return
    try {
      const next = await api.togglePlayer(selectionOf(t), player)
      await apply(selectionOf(next))
    } catch (e) {
      setError(String(e))
    }
  }

  const drag = useDragSwap({
    enabled: editable,
    onDrop: (from, to) => {
      if (t) void apply(swap(selectionOf(t), from, to))
    },
    // Soltar um titular num lugar vazio do campo muda a posição (e a função) dele
    onDropPitch: (from, x, y) => {
      if (!t || from.kind !== 'starter' || t.starters[from.index].role === 'GR') return
      const positions = t.starters.map((s) => [s.x, s.y] as [number, number])
      positions[from.index] = [Math.round(Math.min(96, Math.max(4, x))), Math.round(Math.min(85, Math.max(4, y)))]
      void apply({ ...selectionOf(t), positions, auto: false })
    },
  })

  const cancel = async () => {
    if (changed.current && original.current) {
      try {
        await api.setTactics(original.current)
        const info = await api.gameInfo()
        if (info) game.update(info)
      } catch (e) {
        return setError(String(e))
      }
    }
    nav.back()
  }

  const label = (p: TacticPlayer | null | undefined, fallback: string) => (p ? surname(p.listName) : fallback)
  const toggler = (p: TacticPlayer) => (editable ? () => void toggle(p.id) : undefined)

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={name && `Tácticas ${name}`} fg={colours.fg} bg={colours.bg} />
      <div className="flex shrink-0 items-center gap-1 px-1.5">
        <PurpleMenu
          label="Tácticas"
          options={TACTICS_MENU}
          value={t && !t.custom ? String(t.formationIndex) : undefined}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'instructions') setInstructions(true)
            else if (t) void apply({ ...selectionOf(t), formation: Number(k), positions: [], auto: false })
          }}
        />
        <PurpleMenu label="Vêr" options={[]} disabled />
        <div className="flex-1" />
        {t?.auto && editable && <span className="px-2 text-[12px] text-cm-yellow">O adjunto escolhe a equipa</span>}
        <button className="cm-purple h-6 w-40" disabled title="Chega com o relato dos jogos (3.4)">
          Último Jogo
        </button>
        <PurpleMenu
          label="Editar"
          align="right"
          options={[
            { key: 'auto', label: 'Adjunto escolhe a equipa' },
            { key: 'reset', label: 'Repor as posições da táctica' },
          ]}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'auto' && t) void apply({ ...selectionOf(t), auto: true, positions: [] })
            if (k === 'reset' && t) void apply({ ...selectionOf(t), positions: [] })
          }}
        />
      </div>
      {error && <div className="mx-1.5 mt-1.5 bg-red-900 px-2 py-1 text-white">{error}</div>}
      {t && (
        <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 gap-1.5">
          <div className="flex min-h-0 w-[50%] flex-col">
            <div className="flex h-10 shrink-0 items-center justify-center gap-2 bg-black/70 font-title text-[20px] font-bold text-cm-yellow">
              {t.formation}
              {t.custom && '*'}
              {t.coach && (
                <span className="text-[14px] font-normal text-white">
                  (
                  <Link onClick={() => nav.go({ kind: 'person', id: t.coachId! })} className="text-cm-magenta">
                    {t.coach}
                  </Link>
                  )
                </span>
              )}
            </div>
            <div className="min-h-0 flex-1 overflow-auto bg-black/50 py-1">
              {t.starters.map((s, i) => {
                const spot: Spot = { kind: 'starter', index: i }
                return s.player ? (
                  <Row
                    key={`s${i}`}
                    p={s.player}
                    kind="starter"
                    place={shortRole(s.role)}
                    bind={drag.bind(spot, label(s.player, s.role))}
                    onToggle={toggler(s.player)}
                  />
                ) : (
                  <div key={`s${i}`} {...drag.bind(spot, s.role)} className="flex h-[19px] items-center gap-2 px-1 text-[13px]">
                    <span className="w-7 shrink-0" />
                    <span className="w-9 shrink-0 bg-[#262626] text-center text-[11px] text-cm-yellow">{shortRole(s.role)}</span>
                    <span className="italic text-[#8a8a8a]">— vazio: arraste alguém para aqui —</span>
                  </div>
                )
              })}
              <div className="my-0.5 border-t border-cm-line" />
              {/* Banco fixo de 9: as casas vazias também recebem jogadores arrastados */}
              {Array.from({ length: editable ? BENCH : t.bench.length }, (_, i) => {
                const p = t.bench[i]
                const spot: Spot = { kind: 'bench', index: i }
                return p ? (
                  <Row key={`b${p.id}`} p={p} kind="bench" place={`SB${i + 1}`} bind={drag.bind(spot, label(p, ''))} onToggle={toggler(p)} />
                ) : (
                  <div key={`b-empty${i}`} {...drag.bind(spot, `SB${i + 1}`)} className="flex h-[19px] items-center gap-2 px-1 text-[13px]">
                    <span className="w-7 shrink-0 bg-[#1d2d8a]" />
                    <span className="w-9 shrink-0 bg-[#262626] text-center text-[11px] text-cm-yellow">SB{i + 1}</span>
                    <span className="italic text-[#6a5a8a]">— vazio —</span>
                  </div>
                )
              })}
              {editable && t.reserves.length > 0 && (
                <>
                  <div className="my-0.5 border-t border-cm-line" />
                  {t.reserves.map((p) => (
                    <Row
                      key={`r${p.id}`}
                      p={p}
                      kind="reserve"
                      place=""
                      bind={drag.bind({ kind: 'reserve', id: p.id }, label(p, ''))}
                      onToggle={toggler(p)}
                    />
                  ))}
                </>
              )}
            </div>
          </div>
          <div className="flex min-h-0 flex-1 flex-col">
            <Tabs
              tabs={[
                { key: 'overview', label: 'Vista Geral' },
                { key: 'with', label: 'Com Bola', disabled: true },
                { key: 'without', label: 'Sem Bola', disabled: true },
              ]}
              active="overview"
              onChange={() => {}}
            />
            <div className="mt-1 flex min-h-0 flex-1 flex-col">
              <Pitch t={t} bind={drag.bind} />
            </div>
            {editable && (
              <div className="shrink-0 pt-1 text-center text-[12px] text-[#aab3c4]">
                Arraste para trocar ou para um lugar vazio do campo · clique no número ou na casa para pôr/tirar · duplo clique abre o perfil
              </div>
            )}
          </div>
        </div>
      )}
      {!t && <div className="flex-1" />}
      {drag.ghost}
      {instructions && t && (
        <InstructionsDialog
          initial={t.instructions}
          onCancel={() => setInstructions(false)}
          onOk={(i) => {
            setInstructions(false)
            void apply({ ...selectionOf(t), instructions: i })
          }}
        />
      )}
      <BottomBar left="Cancelar" right="Ok" onLeft={() => void cancel()} onRight={() => nav.back()} />
    </div>
  )
}
