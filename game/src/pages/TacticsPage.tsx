import { ArrowLeftRight, ChevronDown, History, Pencil } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { Selection } from '../bindings/Selection'
import type { TacticPlayer } from '../bindings/TacticPlayer'
import InstructionsDialog from '../components/InstructionsDialog'
import PlayerOrdersDialog from '../components/PlayerOrdersDialog'
import { BottomBar, Link, PurpleMenu } from '../components/Screen'
import { Pitch, Row, RowHeader } from '../components/TacticsBoard'
import { MENTALITY, PASSING, TACKLING, TACTICS_MENU, fullName, moveOnPitch, ordersOf, shortRole, surname, useRunArrow, withOrders } from '../tactics'
import { initials } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'
import { BENCH, sameSpot, selectionOf, swap, useDragSwap, type Spot } from '../selection'

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
  // Jogador com o diálogo das instruções individuais aberto
  const [ordersFor, setOrdersFor] = useState<TacticPlayer | null>(null)
  // A escolha ao abrir a tela, para o Cancelar
  const original = useRef<Selection | null>(null)
  // Troca em dois cliques (Substituir no banco, ⇄ no onze): o primeiro escolhido, à espera do outro
  const [pending, setPending] = useState<Spot | null>(null)
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

  // Seta de movimento a ser desenhada com o botão direito
  const arrow = useRunArrow(t, (sel) => void apply(sel))

  const drag = useDragSwap({
    enabled: editable,
    // Com um jogador escolhido (Substituir ou ⇄), clicar noutro (lista ou campo) troca os dois
    onClick: (spot) => {
      if (!pending || !t || sameSpot(pending, spot)) return
      setPending(null)
      void apply(swap(selectionOf(t), pending, spot))
    },
    onDrop: (from, to) => {
      if (t) void apply(swap(selectionOf(t), from, to))
    },
    // Soltar um titular num lugar vazio do campo muda a posição (e a função) dele
    onDropPitch: (from, x, y) => {
      if (!t || from.kind !== 'starter') return
      const sel = moveOnPitch(t, from.index, x, y)
      if (sel) void apply(sel)
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

  const headButton =
    'flex h-[34px] items-center gap-1.5 rounded-md bg-[#1e1f25] px-3 text-[13px] font-semibold text-[#4edea3] hover:bg-[#292a2f] disabled:text-white/25 disabled:hover:bg-[#1e1f25]'
  const picking = pending !== null
  const hint = pending?.kind === 'starter' ? 'Clique em quem entra' : 'Clique no titular que sai'
  /** Escolhe (ou larga) o primeiro jogador da troca. */
  const choose = (spot: Spot) => setPending((cur) => (sameSpot(cur, spot) ? null : spot))
  const mark = (spot: Spot) => (sameSpot(pending, spot) ? 'wait' : picking ? 'pick' : undefined)

  return (
    <div className="flex h-full flex-col">
      {/* Cabeçalho: o nome da táctica abre o menu Tácticas; as vistas; Vêr, Último Jogo e Editar */}
      <div className="mx-2.5 mt-2.5 flex shrink-0 items-center gap-3 rounded-md bg-[#1a1b21] px-3 py-2.5">
        <PurpleMenu
          label="Tácticas"
          options={TACTICS_MENU}
          value={t && !t.custom ? String(t.formationIndex) : undefined}
          disabled={!editable}
          onChange={(k) => {
            if (k === 'instructions') setInstructions(true)
            else if (t) void apply({ ...selectionOf(t), formation: Number(k), positions: [], auto: false })
          }}
          buttonClass="flex items-center gap-2.5 rounded-md py-1 pl-1 pr-2.5 text-left hover:bg-white/[0.06] disabled:hover:bg-transparent"
          trigger={
            <>
              <span
                className="flex size-10 shrink-0 items-center justify-center rounded font-title text-[16px] font-bold"
                style={{ color: colours.fg, background: colours.bg }}
              >
                {initials(name)}
              </span>
              <span className="min-w-0">
                <span className="flex items-center gap-2 text-[10px] font-bold font-mono uppercase tracking-[0.12em] text-[#c8c8d0]">
                  Tácticas {name}
                  <span className="rounded bg-[#0f3a2a] px-1.5 tracking-wider text-[#4edea3]">{t?.auto ? 'O adjunto escolhe' : 'Táctica activa'}</span>
                </span>
                <span className="block font-title text-[22px] font-bold leading-tight text-cm-orange">
                  {t?.formation}
                  {t?.custom && '*'}
                </span>
              </span>
              {editable && <ChevronDown size={16} className="text-[#86869a]" />}
            </>
          }
        />
        <div className="ml-auto flex gap-0.5 rounded-md bg-black/60 p-[3px]">
          {(
            [
              ['Vista Geral', false],
              ['Com Bola', true],
              ['Sem Bola', true],
            ] as const
          ).map(([label, off], i) => (
            <button
              key={label}
              disabled={off}
              className={`h-7 rounded px-3.5 text-[12px] font-semibold ${i === 0 ? 'bg-cm-purple text-white' : 'text-[#c8c8d0] disabled:text-white/25'}`}
            >
              {label}
            </button>
          ))}
        </div>
        <PurpleMenu label="Vêr" options={[]} disabled buttonClass={headButton} trigger={<>Vêr <ChevronDown size={14} /></>} />
        <button className={headButton} disabled title="Chega com o relato dos jogos (3.4)">
          <History size={14} /> Último Jogo
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
          buttonClass={headButton}
          trigger={
            <>
              <Pencil size={14} /> Editar <ChevronDown size={14} />
            </>
          }
        />
      </div>
      {error && <div className="mx-2.5 mt-2.5 rounded bg-red-900 px-2 py-1 text-white">{error}</div>}
      {t && (
        <div className="mx-2.5 mt-2.5 flex min-h-0 flex-1 gap-2.5">
          {/* Onze, banco e resto do plantel numa coluna só, que rola */}
          <div className="flex min-h-0 w-[54%] flex-col gap-2.5 overflow-auto">
            <div className="shrink-0 overflow-hidden rounded-md bg-[#1a1b21]">
              <PanelTitle
                title="Onze Titular"
                chip={`${t.starters.filter((s) => s.player).length}/11`}
                right={
                  pending ? (
                    <span className="font-bold text-cm-yellow">{hint}</span>
                  ) : (
                    t.coach && (
                      <Link onClick={() => nav.go({ kind: 'person', id: t.coachId! })} className="text-cm-magenta">
                        {t.coach}
                      </Link>
                    )
                  )
                }
              />
              <RowHeader place="Pos" action={editable ? 'Troca' : undefined} />
              <div className="cm-zebra">
                {t.starters.map((s, i) => {
                  const spot: Spot = { kind: 'starter', index: i }
                  return s.player ? (
                    <Row
                      key={`s${i}`}
                      p={s.player}
                      kind="starter"
                      place={shortRole(s.role)}
                      bind={drag.bind(spot, label(s.player, s.role))}
                      onToggle={picking ? undefined : toggler(s.player)}
                      onOpen={() => nav.go({ kind: 'person', id: s.player!.id })}
                      onOrders={editable ? () => setOrdersFor(s.player!) : undefined}
                      ordered={t.orders.some((o) => o.personId === s.player!.id)}
                      captain={t.takers.captain === s.player.id}
                      highlight={mark(spot)}
                      action={
                        editable && (
                          <span className="flex w-[84px] shrink-0 justify-end">
                            <button
                              onMouseDown={(e) => e.stopPropagation()}
                              onClick={() => choose(spot)}
                              title="Troca rápida: escolha depois quem entra"
                              className={`flex h-[22px] w-8 items-center justify-center rounded-[3px] ${
                                sameSpot(pending, spot) ? 'bg-cm-yellow text-black' : 'text-[#c8c8d0] hover:bg-white/10 hover:text-white'
                              }`}
                            >
                              <ArrowLeftRight size={15} />
                            </button>
                          </span>
                        )
                      }
                    />
                  ) : (
                    <div
                      key={`s${i}`}
                      {...drag.bind(spot, s.role)}
                      className={`flex h-[30px] items-center gap-2 px-3 text-[14px] ${picking ? 'cursor-pointer hover:!bg-cm-yellow/15' : ''}`}
                    >
                      <span className="w-8 shrink-0" />
                      <span className="w-12 shrink-0 text-[11px] font-bold text-cm-yellow">{shortRole(s.role)}</span>
                      <span className="italic text-[#8a8a8a]">— vazio: arraste alguém para aqui —</span>
                    </div>
                  )
                })}
              </div>
            </div>
            <div className="shrink-0 overflow-hidden rounded-md bg-[#1a1b21]">
              <PanelTitle title="Banco de Suplentes" chip="SB1 – SB9" right={pending && <span className="font-bold text-cm-yellow">{hint}</span>} />
              <RowHeader place="Slot" action={editable ? 'Acção' : undefined} />
              {/* Banco fixo de 9: as casas vazias também recebem jogadores arrastados */}
              <div className="cm-zebra">
                {Array.from({ length: editable ? BENCH : t.bench.length }, (_, i) => {
                  const p = t.bench[i]
                  const spot: Spot = { kind: 'bench', index: i }
                  return p ? (
                    <Row
                      key={`b${p.id}`}
                      p={p}
                      kind="bench"
                      place={`SB${i + 1}`}
                      bind={drag.bind(spot, label(p, ''))}
                      onToggle={picking ? undefined : toggler(p)}
                      onOpen={() => nav.go({ kind: 'person', id: p.id })}
                      highlight={mark(spot)}
                      action={
                        editable && (
                          <button
                            onMouseDown={(e) => e.stopPropagation()}
                            onClick={() => choose(spot)}
                            className={`w-[84px] shrink-0 rounded-[3px] text-[11px] font-bold uppercase leading-[20px] tracking-wide ${
                              sameSpot(pending, spot) ? 'bg-cm-yellow text-black' : 'bg-cm-purple text-white hover:bg-cm-purplelight'
                            }`}
                          >
                            {sameSpot(pending, spot) ? 'Cancelar' : 'Substituir'}
                          </button>
                        )
                      }
                    />
                  ) : (
                    <div key={`b-empty${i}`} {...drag.bind(spot, `SB${i + 1}`)} className="flex h-[30px] items-center gap-2 px-3 text-[14px]">
                      <span className="w-8 shrink-0" />
                      <span className="w-12 shrink-0 text-[11px] font-bold text-[#d0bcff]">SB{i + 1}</span>
                      <span className="italic text-[#6a5a8a]">— vazio —</span>
                    </div>
                  )
                })}
              </div>
            </div>
            {editable && t.reserves.length > 0 && (
              <div className="shrink-0 overflow-hidden rounded-md bg-[#1a1b21]">
                <PanelTitle title="Resto do Plantel" />
                <RowHeader place="" />
                <div className="cm-zebra">
                  {t.reserves.map((p) => {
                    const spot: Spot = { kind: 'reserve', id: p.id }
                    return (
                      <Row
                        key={`r${p.id}`}
                        p={p}
                        kind="reserve"
                        place=""
                        bind={drag.bind(spot, label(p, ''))}
                        onToggle={picking ? undefined : toggler(p)}
                        onOpen={() => nav.go({ kind: 'person', id: p.id })}
                        highlight={mark(spot)}
                      />
                    )
                  })}
                </div>
              </div>
            )}
          </div>
          {/* O campo com toda a altura e, ao lado, o resumo das instruções */}
          <div className="flex min-h-0 min-w-0 flex-1 gap-2.5">
            <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-md bg-[#1a1b21]">
              <PanelTitle
                title="Quadro Táctico"
                chip={`${t.formation}${t.custom ? '*' : ''}`}
                right={
                  editable && (
                    <span
                      className="cursor-help text-[#86869a]"
                      title="Arraste, ⇄ ou Substituir para trocar · botão direito e arrastar faz a seta (atacar/recuar) · clique no número para pôr/tirar · botão direito num jogador da lista: instruções individuais"
                    >
                      Como mexer ?
                    </span>
                  )
                }
              />
              <div className="flex min-h-0 flex-1 flex-col p-3">
                <Pitch t={t} bind={drag.bind} run={arrow.run} onRunStart={arrow.start} />
              </div>
            </div>
            <InstructionsSummary t={t} onEdit={editable ? () => setInstructions(true) : undefined} />
          </div>
        </div>
      )}
      {!t && <div className="flex-1" />}
      {drag.ghost}
      {instructions && t && (
        <InstructionsDialog
          initial={t.instructions}
          takers={t.takers}
          players={t.starters.flatMap((s) => (s.player ? [{ id: s.player.id, name: fullName(s.player.listName) }] : []))}
          onCancel={() => setInstructions(false)}
          onOk={(i, takers) => {
            setInstructions(false)
            void apply({ ...selectionOf(t), instructions: i, takers })
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
            void apply(withOrders(selectionOf(t), ordersFor.id, o))
          }}
        />
      )}
      <BottomBar left="Cancelar" right="Ok" onLeft={() => void cancel()} onRight={() => nav.back()} />
    </div>
  )
}

/** Barra de título de um painel: nome em laranja, uma etiqueta verde e algo à direita. */
function PanelTitle({ title, chip, right }: { title: string; chip?: string; right?: ReactNode }) {
  return (
    <div className="flex h-9 items-center gap-2 border-b border-cm-line px-3 text-[12px] font-bold font-mono uppercase tracking-[0.12em] text-cm-orange">
      {title}
      {chip && <span className="rounded bg-[#0f3a2a] px-1.5 tracking-wider text-[#4edea3]">{chip}</span>}
      <span className="ml-auto truncate text-[12px] normal-case tracking-normal">{right}</span>
    </div>
  )
}

/** Resumo das Instruções à Equipa ao lado do campo; Editar abre o diálogo de sempre. */
function InstructionsSummary({ t, onEdit }: { t: ClubTactics; onEdit?: () => void }) {
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
    <div className="flex w-[176px] shrink-0 flex-col overflow-hidden rounded-md bg-[#1a1b21]">
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
