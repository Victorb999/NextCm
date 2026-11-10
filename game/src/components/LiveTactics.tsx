import { useEffect, useState } from 'react'
import { api } from '../api'
import { useNav } from '../nav'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { LiveView } from '../bindings/LiveView'
import type { Selection } from '../bindings/Selection'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import { selectionOf, swap, useDragSwap, type Spot } from '../selection'
import InstructionsDialog from './InstructionsDialog'
import PlayerOrdersDialog from './PlayerOrdersDialog'
import { BottomBar, ErrorBox, Link, PanelTitle, PurpleMenu, ScreenHeader } from './Screen'
import { InstructionsSummary, Pitch, Row, RowHeader } from './TacticsBoard'
import { TACTICS_MENU, TACTICS_VIEWS, fullName, hasOrders, moveOnPitch, ordersOf, shortRole, surname, useRunArrow, viewLabel, viewValue, withOrders, type TacticsView } from '../tactics'

/**
 * "Tácticas <clube>" a meio do jogo: a mesma tela de Tácticas (arrastar na lista ou no
 * campo, formações e Instruções à Equipa no menu Tácticas, posições livres, setas com o
 * botão direito), só com quem está no jogo. Trocar um titular com um suplente é uma
 * substituição. Nada vai para o jogo até ao Ok; o Cancelar esquece tudo.
 */
export default function LiveTactics({
  view,
  side,
  notice,
  onClose,
  onApplied,
}: {
  view: LiveView
  side: 0 | 1
  /** Um aviso em cima (o lesionado que fez parar o jogo). */
  notice?: string | null
  onClose: () => void
  onApplied: (v: LiveView) => void
}) {
  const [t, setT] = useState<ClubTactics | null>(null)
  // A nota de cada um na partida (em campo, no banco e quem já saiu)
  const live = side === 0 ? view.home : view.away
  const ratings = new Map([...live.players, ...live.bench, ...live.out].map((p) => [p.id, p.rating]))
  // As cores do clube nos quadrados do campo
  const teamClub = side === 0 ? view.home.club : view.away.club
  const colours = teamClub.fg && teamClub.bg ? { fg: teamClub.fg, bg: teamClub.bg } : undefined
  const [error, setError] = useState<string | null>(null)
  const [instructions, setInstructions] = useState(false)
  const [ordersFor, setOrdersFor] = useState<TacticPlayer | null>(null)
  // O Vêr: o que a coluna da direita da lista mostra (posição, condição, golos, instruções...)
  const [viewing, setViewing] = useState<TacticsView>('position')
  const col = (p: TacticPlayer) => (viewing === 'position' || !t ? undefined : viewValue(viewing, p, ordersOf(t, p.id)))
  const [busy, setBusy] = useState(false)
  // Como estava ao abrir: quem estava em campo (para contar as substituições) e a escolha
  const [original, setOriginal] = useState<{ on: number[]; sel: Selection } | null>(null)
  const team = side === 0 ? view.home : view.away
  const nav = useNav()
  /** "Lautaro 23', Thuram 40'": os golos de um lado pelo relato. */
  const scorers = (s: 0 | 1) => {
    const t = s === 0 ? view.home : view.away
    const name = (id: number | null) => surname([...t.players, ...t.bench, ...t.out].find((p) => p.id === id)?.listName ?? '?')
    return view.events
      .filter((e) => e.kind === 'goal' && e.side === s)
      .map((e) => `${name(e.playerId)} ${e.minute}'`)
      .join(', ')
  }
  /** "ent 46'": quem entrou do banco durante o jogo, como na ficha da partida. */
  const entered = (id: number) => {
    const m = live.players.find((p) => p.id === id)?.onMinute
    return m !== null && m !== undefined ? <span className="text-cm-orange">ent {m}'</span> : undefined
  }
  const editable = !!t?.editable

  useEffect(() => {
    api.liveTactics(side).then(
      (tt) => {
        setOriginal({ on: tt.starters.flatMap((s) => (s.player ? [s.player.id] : [])), sel: selectionOf(tt) })
        setT(tt)
      },
      (e) => setError(String(e)),
    )
  }, [side])

  /** Mostra como fica (funções e notas vêm do core), sem mexer no jogo. */
  const preview = async (sel: Selection) => {
    try {
      setT(await api.livePreviewTactics(sel))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }

  const arrow = useRunArrow(t, (sel) => void preview(sel))
  const drag = useDragSwap({
    enabled: editable,
    onDrop: (from: Spot, to: Spot) => {
      if (t) void preview(swap(selectionOf(t), from, to))
    },
    onDropPitch: (from, x, y) => {
      if (!t || from.kind !== 'starter') return
      const sel = moveOnPitch(t, from.index, x, y)
      if (sel) void preview(sel)
    },
  })

  const entering = t && original ? t.starters.filter((s) => s.player && !original.on.includes(s.player.id)).length : 0
  const subsLeft = view.minute === 0 ? team.subsLeft : team.subsLeft - entering

  const ok = async () => {
    if (!t || busy) return
    const sel = selectionOf(t)
    if (!editable || JSON.stringify(sel) === JSON.stringify(original?.sel)) return onClose()
    setBusy(true)
    try {
      onApplied(await api.liveSetTactics(sel))
      onClose()
    } catch (e) {
      setError(String(e))
      setBusy(false)
    }
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (instructions) return
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onClose()
      } else if (e.key === 'Enter') {
        e.stopImmediatePropagation()
        e.preventDefault()
        void ok()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  const label = (s: { player: { listName: string } | null; role: string }) => (s.player ? surname(s.player.listName) : s.role)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <ScreenHeader
        club={team.club}
        eyebrow={
          // O nome do clube abre a ficha dele
          <Link onClick={() => nav.go({ kind: 'club', id: team.club.id })}>{t?.clubName ?? team.club.name}</Link>
        }
        title="Tácticas"
        right={
          // O resultado e quem marcou (com o minuto), como no placar da partida
          <div className="flex flex-col items-end gap-0.5">
            <span className="rounded bg-black/40 px-3 font-title text-[18px] font-bold text-white">
              {view.home.club.shortName} <span className="text-cm-yellow">{view.homeGoals}-{view.awayGoals}</span> {view.away.club.shortName}
            </span>
            {([0, 1] as const).map((s) => {
              const goals = scorers(s)
              return goals ? (
                <span key={s} className="text-[12px] text-[#c8c8d0]">
                  <b className="text-white">{(s === 0 ? view.home : view.away).club.shortName}:</b> {goals}
                </span>
              ) : null
            })}
          </div>
        }
      />
      <div className="flex shrink-0 items-center gap-1 px-1.5">
        <PurpleMenu
          label="Tácticas"
          options={TACTICS_MENU.filter((o) => o.key !== 'load' && o.key !== 'save')}
          value={t && !t.custom ? String(t.formationIndex) : undefined}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'instructions') setInstructions(true)
            else if (t) void preview({ ...selectionOf(t), formation: Number(k), positions: [] })
          }}
        />
        <PurpleMenu label="Vêr" options={TACTICS_VIEWS} value={viewing} onChange={setViewing} />
        <div className="flex-1" />
        {editable && (
          <span className="px-2 text-[12px] text-cm-yellow">
            {view.minute === 0 ? 'Antes do apito as trocas não contam' : `Substituições restantes: ${subsLeft}`}
          </span>
        )}
        <PurpleMenu
          label="Editar"
          align="right"
          options={[{ key: 'reset', label: 'Repor as posições da táctica' }]}
          disabled={!editable}
          onChange={() => {
            if (t) void preview({ ...selectionOf(t), positions: [] })
          }}
        />
      </div>
      {notice && (
        <div className="mx-2 mt-2 rounded bg-[#5a1414] px-2 py-1 text-center font-bold text-white">{notice}</div>
      )}
      <ErrorBox className="mx-2 mt-2">{error}</ErrorBox>
      {t ? (
        <div className="mx-2 mt-2 flex min-h-0 flex-1 gap-1.5">
          {/* Onze, banco e quem já saiu numa coluna que rola, nos painéis da tela de Tácticas */}
          <div className="flex min-h-0 w-[50%] flex-col gap-2 overflow-y-auto overflow-x-hidden">
            <div className="shrink-0 overflow-hidden rounded-md bg-cm-surface">
              <PanelTitle title="Onze Titular" chip={`${t.formation}${t.custom ? '*' : ''}`} />
              <RowHeader place="Pos" rating column={viewLabel(viewing)} />
              <div className="cm-zebra">
                {t.starters.map((s, i) => {
                  const spot: Spot = { kind: 'starter', index: i }
                  return s.player ? (
                    <Row
                      onOpen={() => nav.go({ kind: 'person', id: s.player!.id })}
                      key={`s${i}`}
                      p={s.player}
                      column={col(s.player)}
                      kind="starter"
                      place={shortRole(s.role)}
                      bind={drag.bind(spot, label(s))}
                      onOrders={editable ? () => setOrdersFor(s.player!) : undefined}
                      ordered={hasOrders(t, s.player!.id)}
                      captain={t.takers.captain === s.player.id}
                      rating={ratings.get(s.player.id) ?? null}
                      note={entered(s.player.id)}
                    />
                  ) : (
                    <div key={`s${i}`} {...drag.bind(spot, s.role)} className="flex h-[19px] items-center gap-2 px-1 text-[13px]">
                      <span className="w-7 shrink-0" />
                      <span className="cm-cell w-9 shrink-0 text-center text-[11px] text-cm-yellow">{shortRole(s.role)}</span>
                      <span className="italic text-[#8a8a8a]">— sem jogador (expulso) —</span>
                    </div>
                  )
                })}
              </div>
            </div>
            <div className="shrink-0 overflow-hidden rounded-md bg-cm-surface">
              <PanelTitle title="Banco de Suplentes" chip={`${t.bench.length}`} />
              <RowHeader place="Slot" rating column={viewLabel(viewing)} />
              <div className="cm-zebra">
                {t.bench.map((p, i) => {
                  // Quem estava em campo e foi para o banco nesta tela sai de vez no Ok (antes do apito volta ao banco)
                  const leaving = view.minute > 0 && !!original?.on.includes(p.id)
                  return leaving ? (
                    <Row
                      onOpen={() => nav.go({ kind: 'person', id: p.id })}
                      key={`b${p.id}`}
                      p={p}
                      column={col(p)}
                      kind="reserve"
                      place="Sai"
                      bind={drag.bind({ kind: 'bench', index: i }, surname(p.listName))}
                      rating={ratings.get(p.id) ?? null}
                      note="sai no Ok"
                    />
                  ) : (
                    <Row
                      onOpen={() => nav.go({ kind: 'person', id: p.id })}
                      key={`b${p.id}`}
                      p={p}
                      column={col(p)}
                      kind="bench"
                      place={`SB${i + 1}`}
                      bind={drag.bind({ kind: 'bench', index: i }, surname(p.listName))}
                      rating={ratings.get(p.id) ?? null}
                    />
                  )
                })}
              </div>
            </div>
            {/* Quem já saiu não pode voltar: só aparece, sem se poder arrastar */}
            {team.out.length > 0 && (
              <div className="shrink-0 overflow-hidden rounded-md bg-cm-surface">
                <PanelTitle title="Já Saíram" chip={`${team.out.length}`} />
                <div className="cm-zebra">
                  {team.out.map((p) => (
                    <div key={`o${p.id}`} className="flex h-[30px] items-center gap-2 px-3 text-[14px] text-[#6a6a6a]">
                      <span className="w-8 shrink-0 text-center">{p.squadNumber ?? ''}</span>
                      <span className="w-12 shrink-0 text-[11px] font-bold">{p.red ? 'Exp' : 'Saiu'}</span>
                      <span className="flex-1 truncate">{p.listName.split(', ').reverse().join(' ')}</span>
                      <span className="shrink-0 text-[12px]">{p.red ? `expulso ${p.offMinute}'` : `saiu ${p.offMinute}'`}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
          <div className="flex min-h-0 flex-1 gap-2">
            <div className="flex min-h-0 flex-1 flex-col">
              <div className="flex min-h-0 flex-1 flex-col">
                <Pitch t={t} colours={colours} bind={drag.bind} run={arrow.run} onRunStart={arrow.start} />
              </div>
              <div className="shrink-0 pt-1 text-center text-[12px] text-[#aab3c4]">
                {editable
                  ? 'Arraste para trocar (titular com suplente = substituição) ou para um lugar vazio · botão direito e arrastar faz a seta'
                  : 'Só pode mexer na sua equipa.'}
              </div>
            </div>
            {/* As Instruções à Equipa à vista, como na tela de Tácticas; Editar abre o diálogo */}
            <InstructionsSummary t={t} onEdit={editable ? () => setInstructions(true) : undefined} />
          </div>
        </div>
      ) : (
        <div className="flex-1" />
      )}
      {drag.ghost}
      {instructions && t && (
        <InstructionsDialog
          initial={t.instructions}
          takers={t.takers}
          players={t.starters.flatMap((s) => (s.player ? [{ id: s.player.id, name: fullName(s.player.listName) }] : []))}
          onCancel={() => setInstructions(false)}
          onOk={(i, takers) => {
            setInstructions(false)
            void preview({ ...selectionOf(t), instructions: i, takers })
          }}
        />
      )}
      {ordersFor && t && (
        <PlayerOrdersDialog
          title={`${ordersFor.squadNumber ?? ''} ${fullName(ordersFor.listName)}`}
          initial={ordersOf(t, ordersFor.id)}
          onCancel={() => setOrdersFor(null)}
          onOk={(o) => {
            setOrdersFor(null)
            void preview(withOrders(selectionOf(t), ordersFor.id, o))
          }}
        />
      )}
      <BottomBar left="Cancelar" right="Ok" onLeft={onClose} onRight={busy ? undefined : () => void ok()} />
    </div>
  )
}
