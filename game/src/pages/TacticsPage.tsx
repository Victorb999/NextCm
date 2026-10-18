import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { Selection } from '../bindings/Selection'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import InstructionsDialog from '../components/InstructionsDialog'
import { BottomBar, Link, PurpleMenu, Tabs, TitleBand } from '../components/Screen'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// Formações na ordem do core (FORMATIONS em core/src/tactics.rs); a 0 ("não definida") não aparece
const FORMATIONS = ['4-4-2', '5-3-2 Líbero', '5-3-2', '4-4-2', '4-3-3', '4-2-4', '3-5-2', '4-1-2-1-2', '4-5-1', '3-4-3']
const FORMATION_OPTIONS = FORMATIONS.map((label, i) => ({ key: String(i), label })).slice(1)

/** "Sobrenome, Nome" -> "Sobrenome" (como os nomes no campo do CM). */
const surname = (listName: string) => listName.split(', ')[0]

/** Nome na lista: "Nome Sobrenome", como na tela de tácticas do CM. */
const fullName = (listName: string) => {
  const [last, first] = listName.split(', ')
  return first ? `${first} ${last}` : last
}

/** Lugar de um jogador na escolha: titular (função), suplente ou reserva. */
type Spot = { kind: 'starter'; index: number } | { kind: 'bench'; index: number } | { kind: 'reserve'; id: number }

const sameSpot = (a: Spot | null, b: Spot) =>
  !!a && a.kind === b.kind && (a.kind === 'reserve' ? a.id === (b as { id: number }).id : a.index === (b as { index: number }).index)

const selectionOf = (t: ClubTactics): Selection => ({
  formation: t.formationIndex,
  starters: t.starters.map((s) => s.player?.id ?? null),
  bench: t.bench.map((p) => p.id),
  instructions: t.instructions,
  auto: t.auto,
})

function idAt(sel: Selection, spot: Spot): number | null {
  if (spot.kind === 'starter') return sel.starters[spot.index] ?? null
  if (spot.kind === 'bench') return sel.bench[spot.index] ?? null
  return spot.id
}

/** Troca dois jogadores de lugar, como no CM. Quem sai do onze ou do banco vai para as reservas. */
function swap(sel: Selection, a: Spot, b: Spot): Selection {
  const next: Selection = { ...sel, starters: [...sel.starters], bench: [...sel.bench], auto: false }
  const ia = idAt(sel, a)
  const ib = idAt(sel, b)
  const put = (spot: Spot, id: number | null) => {
    if (spot.kind === 'starter') next.starters[spot.index] = id
    else if (spot.kind === 'bench') {
      if (id === null) next.bench.splice(spot.index, 1)
      else next.bench[spot.index] = id
    }
  }
  // Reserva ↔ reserva não muda nada
  if (a.kind === 'reserve' && b.kind === 'reserve') return sel
  // Pôr primeiro o lugar de índice maior no banco, para o splice não deslocar o outro
  const order: [Spot, number | null][] = [
    [a, ib],
    [b, ia],
  ]
  order.sort(([x], [y]) => (y.kind === 'bench' ? y.index : -1) - (x.kind === 'bench' ? x.index : -1))
  for (const [spot, id] of order) put(spot, id)
  return next
}

function conditionColour(c: number | null) {
  if (c === null) return 'text-cm-yellow'
  if (c >= 90) return 'text-cm-yellow'
  if (c >= 75) return 'text-cm-orange'
  return 'text-[#ff4040]'
}

function Row({
  p,
  kind,
  role,
  selected,
  onClick,
}: {
  p: TacticPlayer
  kind: 'starter' | 'bench' | 'reserve'
  role?: string
  selected: boolean
  onClick: () => void
}) {
  const nav = useNav()
  const out = kind === 'starter' && p.fit !== null && p.fit < 15
  const text = kind === 'bench' ? 'text-[#8a5ad8]' : kind === 'reserve' ? 'text-[#8a8a8a]' : out ? 'text-[#ff5050]' : 'text-white'
  return (
    <div
      onMouseDown={onClick}
      onDoubleClick={() => nav.go({ kind: 'person', id: p.id })}
      className={`flex h-[19px] cursor-pointer items-center gap-2 px-1 text-[13.5px] ${selected ? 'bg-cm-sel' : ''}`}
      title={role ? `${role}${p.fit !== null ? ` · nota ${p.fit}` : ''}` : undefined}
    >
      <span className={`w-7 shrink-0 text-center tabular-nums ${kind === 'bench' ? 'bg-[#1d2d8a] text-white' : 'text-white'}`}>
        {p.squadNumber ?? ''}
      </span>
      <span className={`flex-1 truncate ${selected ? 'text-white' : text}`}>{fullName(p.listName)}</span>
      {(p.injured || p.suspended) && (
        <span className="shrink-0 bg-[#9c0000] px-1 text-[11px] font-bold text-white">{p.injured ? 'Les' : 'Sus'}</span>
      )}
      <span className={`w-24 shrink-0 truncate ${selected ? 'text-white' : text}`}>{p.position}</span>
      <span className={`w-10 shrink-0 text-right ${selected ? 'text-white' : conditionColour(p.condition)}`}>
        {p.condition ?? 100}%
      </span>
    </div>
  )
}

/** Campo com a formação, como a "Vista Geral" do CM. */
function Pitch({ t, selected, onPick }: { t: ClubTactics; selected: Spot | null; onPick: (s: Spot) => void }) {
  return (
    <div className="relative min-h-0 flex-1 overflow-hidden border border-[#1d5a1d] bg-[repeating-linear-gradient(0deg,#1f6b22_0_36px,#1c6420_36px_72px)]">
      {/* Linhas do campo */}
      <div className="pointer-events-none absolute inset-2 border border-white/40" />
      <div className="pointer-events-none absolute left-2 right-2 top-1/2 border-t border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-1/2 h-24 w-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white/40" />
      <div className="pointer-events-none absolute left-1/2 top-2 h-[16%] w-[44%] -translate-x-1/2 border border-t-0 border-white/40" />
      <div className="pointer-events-none absolute bottom-2 left-1/2 h-[16%] w-[44%] -translate-x-1/2 border border-b-0 border-white/40" />
      {t.starters.map((s, i) => {
        const spot: Spot = { kind: 'starter', index: i }
        const sel = sameSpot(selected, spot)
        const out = s.player?.fit !== null && s.player?.fit !== undefined && s.player.fit < 15
        return (
          <div
            key={i}
            onMouseDown={() => t.editable && onPick(spot)}
            className={`absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center ${t.editable ? 'cursor-pointer' : ''}`}
            style={{ left: `${s.x}%`, top: `${s.y}%` }}
            title={s.role}
          >
            <span
              className={`flex h-6 w-6 items-center justify-center rounded-full border text-[12px] font-bold ${
                sel ? 'border-cm-yellow bg-cm-sel text-white' : s.role === 'GR' ? 'border-black bg-[#0f9a2a] text-white' : 'border-black bg-white text-black'
              }`}
            >
              {s.player?.squadNumber ?? ''}
            </span>
            <span
              className={`whitespace-nowrap text-[13px] font-bold [text-shadow:1px_1px_0_#000] ${out ? 'text-[#ff6060]' : 'text-cm-yellow'}`}
            >
              {s.player ? surname(s.player.listName) : s.role}
            </span>
          </div>
        )
      })}
    </div>
  )
}

/** Tela "Tácticas <clube>": a do treinador é editável (trocas, formação, instruções); as outras só mostram. */
export default function TacticsPage({ id }: { id: number }) {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [t, setT] = useState<ClubTactics | null>(null)
  const [draft, setDraft] = useState<Selection | null>(null)
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<Spot | null>(null)
  const [instructions, setInstructions] = useState(false)

  useEffect(() => {
    api.getClubTactics(id).then(setT, (e) => setError(String(e)))
    api.getClub(id).then(setClub, () => {})
  }, [id])

  const colours = club?.club.fg && club?.club.bg ? { fg: club.club.fg, bg: club.club.bg } : { fg: '#ffffff', bg: '#3a3a3a' }
  const name = club?.club.shortName ?? t?.clubName ?? ''
  const editable = !!t?.editable

  /** Mostra como fica a equipa com `sel` (o core recalcula as notas na função). */
  const preview = async (sel: Selection) => {
    try {
      const next = await api.previewTactics(sel)
      setT(next)
      setDraft(selectionOf(next))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }

  const pick = (spot: Spot) => {
    if (!t || !editable) return
    if (!selected) return setSelected(spot)
    if (sameSpot(selected, spot)) return setSelected(null)
    const from = draft ?? selectionOf(t)
    // Uma reserva só pode trocar com quem está no onze ou no banco
    if (selected.kind === 'reserve' && spot.kind === 'reserve') return setSelected(spot)
    setSelected(null)
    void preview(swap(from, selected, spot))
  }

  const ok = async () => {
    if (draft) {
      try {
        await api.setTactics(draft)
        const info = await api.gameInfo()
        if (info) game.update(info)
      } catch (e) {
        return setError(String(e))
      }
    }
    nav.back()
  }

  const base = t && (draft ?? selectionOf(t))

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={name && `Tácticas ${name}`} fg={colours.fg} bg={colours.bg} />
      <div className="flex shrink-0 items-center gap-1 px-1.5">
        <PurpleMenu
          label="Tácticas"
          options={FORMATION_OPTIONS}
          value={t ? String(t.formationIndex) : undefined}
          disabled={!editable}
          onChange={(k) => base && void preview({ ...base, formation: Number(k), auto: false })}
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
            { key: 'instructions', label: 'Instruções à Equipa' },
            { key: 'auto', label: 'Adjunto escolhe a equipa' },
          ]}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'instructions') setInstructions(true)
            if (k === 'auto' && base) void preview({ ...base, auto: true })
          }}
        />
      </div>
      {error && <div className="mx-1.5 mt-1.5 bg-red-900 px-2 py-1 text-white">{error}</div>}
      {t && (
        <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 gap-1.5">
          <div className="flex min-h-0 w-[50%] flex-col">
            <div className="flex h-10 shrink-0 items-center justify-center gap-2 bg-black/70 font-title text-[20px] font-bold text-cm-yellow">
              {t.formation}
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
                  <Row key={`s${i}`} p={s.player} kind="starter" role={s.role} selected={sameSpot(selected, spot)} onClick={() => pick(spot)} />
                ) : (
                  <div
                    key={`s${i}`}
                    onMouseDown={() => pick(spot)}
                    className={`flex h-[19px] items-center px-9 text-[13px] italic text-[#ff6060] ${sameSpot(selected, spot) ? 'bg-cm-sel' : ''}`}
                  >
                    {s.role} — sem jogador
                  </div>
                )
              })}
              <div className="my-0.5 border-t border-cm-line" />
              {t.bench.map((p, i) => {
                const spot: Spot = { kind: 'bench', index: i }
                return <Row key={`b${p.id}`} p={p} kind="bench" selected={sameSpot(selected, spot)} onClick={() => pick(spot)} />
              })}
              {editable && t.reserves.length > 0 && (
                <>
                  <div className="my-0.5 border-t border-cm-line" />
                  {t.reserves.map((p) => {
                    const spot: Spot = { kind: 'reserve', id: p.id }
                    return <Row key={`r${p.id}`} p={p} kind="reserve" selected={sameSpot(selected, spot)} onClick={() => pick(spot)} />
                  })}
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
              <Pitch t={t} selected={selected} onPick={pick} />
            </div>
            {editable && (
              <div className="shrink-0 pt-1 text-center text-[12px] text-[#aab3c4]">
                Clique num jogador e depois noutro para trocar. Duplo clique abre o perfil.
              </div>
            )}
          </div>
        </div>
      )}
      {!t && <div className="flex-1" />}
      {instructions && t && (
        <InstructionsDialog
          initial={t.instructions}
          onCancel={() => setInstructions(false)}
          onOk={(i) => {
            setInstructions(false)
            if (base) void preview({ ...base, instructions: i })
          }}
        />
      )}
      <BottomBar left="Cancelar" right="Ok" onLeft={nav.canBack ? nav.back : undefined} onRight={() => void ok()} />
    </div>
  )
}
