import { useEffect, useMemo, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ClubProfile } from '../bindings/ClubProfile'
import type { ClubTactics } from '../bindings/ClubTactics'
import type { Fixture } from '../bindings/Fixture'
import { ClubFixtureList, MatchInfo } from '../components/Fixtures'
import FinancesPanel from '../components/FinancesPanel'
import TransfersPanel from '../components/TransfersPanel'
import { BottomBar, Empty, ErrorBox, Link, PurpleMenu, ScreenHeader, Tabs } from '../components/Screen'
import { money, shortListName } from '../format'
import { useGame } from '../game'
import { useNav } from '../nav'
import { BENCH, selectionOf, spotOf, swap, useDragSwap, type Spot } from '../selection'

const TABS = [
  { key: 'squad', label: 'Plantel' },
  { key: 'transfers', label: 'Transferências' },
  { key: 'next', label: 'Próximo Jogo' },
  { key: 'fixtures', label: 'Calendário' },
  { key: 'info', label: 'Finanças & Info' },
]

type View = 'players' | 'reserves' | 'staff'
type Criterion = 'position' | 'name' | 'number' | 'age' | 'ability'

const VIEWS: { key: View; label: string }[] = [
  { key: 'players', label: 'Jogadores' },
  { key: 'reserves', label: 'Reservas' },
  { key: 'staff', label: 'Staff' },
]

const CRITERIA: { key: Criterion; label: string }[] = [
  { key: 'position', label: 'Posição' },
  { key: 'name', label: 'Nome' },
  { key: 'number', label: 'Número' },
  { key: 'age', label: 'Idade' },
  { key: 'ability', label: 'Habilidade' },
]

/** Uma linha do plantel: [número] [lugar/Int] Nome ... posição */
type Entry = { id: number; number: ReactNode; badge: ReactNode; name: string; detail: string; spot?: Spot }

type Bind = { 'data-spot': string; onMouseDown: (e: React.MouseEvent) => void }

/** "D D" -> "DD", "M C" -> "MC": a sigla curta da função, como nas casas do plantel do CM. */
const shortRole = (role: string) => role.replace(' ', '')

const byNullsLast = (a: number | null, b: number | null) => (a ?? 999) - (b ?? 999)

function SquadGrid({
  entries,
  selected,
  onSelect,
  onOpen,
  bind,
  onToggle,
}: {
  entries: Entry[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (e: Entry) => void
  /** Só no clube do treinador: arrastar para mudar o lugar na equipa. */
  bind?: (spot: Spot, label: string) => Bind
  /** Só no clube do treinador: clicar no número ou na casa põe/tira da equipa. */
  onToggle?: (e: Entry) => void
}) {
  const row = (e: Entry, i: number) => {
    const drag = bind && e.spot ? bind(e.spot, e.name) : undefined
    const toggle = onToggle && (() => onToggle(e))
    const stop = onToggle && ((ev: React.MouseEvent) => ev.stopPropagation())
    return (
      <div
        key={e.id}
        data-row={i}
        data-spot={drag?.['data-spot']}
        onMouseDown={drag?.onMouseDown}
        onClick={() => onSelect(i)}
        onDoubleClick={() => onOpen(e)}
        className={`flex h-[26px] items-center gap-1.5 px-2 ${i === selected ? '!bg-cm-sel' : ''} ${drag ? 'cursor-grab' : ''}`}
      >
        <span
          onMouseDown={stop}
          onClick={toggle}
          className={`w-8 shrink-0 text-center font-mono text-[13px] text-[#c8c8d0] ${onToggle ? 'cursor-pointer' : ''}`}
        >
          {e.number}
        </span>
        <span
          onMouseDown={stop}
          onClick={toggle}
          title={onToggle ? 'Clique para pôr/tirar da equipa' : undefined}
          className={`w-10 shrink-0 text-center font-mono text-[12px] font-bold text-[#4edea3] ${onToggle ? 'cursor-pointer' : ''}`}
        >
          {e.badge || <span className="text-[#5a5a66]">-</span>}
        </span>
        <span className="min-w-0 flex-1 truncate text-[15px] font-semibold text-white">
          <Link onClick={() => onOpen(e)}>{e.name}</Link>
        </span>
        <span className={`w-28 shrink-0 truncate text-right font-mono text-[12.5px] ${i === selected ? 'text-white' : 'text-cm-yellow'}`}>{e.detail}</span>
      </div>
    )
  }
  const head = (
    <div className="flex h-7 shrink-0 items-center gap-1.5 border-b border-cm-line bg-black/30 px-2 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-[#86869a]">
      <span className="w-8 text-center">Nº</span>
      <span className="w-10 text-center">Esc</span>
      <span className="flex-1">Jogador</span>
      <span className="w-28 text-right">Posição</span>
    </div>
  )
  const column = (side: 0 | 1) => (
    <div className="min-w-0 flex-1 overflow-hidden rounded-md bg-[#1a1b21]">
      {head}
      <div className="cm-zebra">{entries.map((e, i) => (i % 2 === side ? row(e, i) : null)).filter(Boolean)}</div>
    </div>
  )
  return (
    <div className="flex items-start gap-2.5">
      {column(0)}
      {column(1)}
    </div>
  )
}

/** Finanças pela palavra, como o CM (o banco guarda o saldo em euros). */
function financesLabel(cash: number | null) {
  if (cash === null) return '-'
  if (cash >= 50_000_000) return 'Rico'
  if (cash >= 10_000_000) return 'Seguro'
  if (cash >= 1_000_000) return 'Razoável'
  if (cash >= 0) return 'Apertado'
  return 'Em dívida'
}

/** "Finanças & Info" → Informação Geral do CM: dados do clube e a lista do staff. */
function Info({ c, finances }: { c: ClubProfile; finances?: ReactNode }) {
  const nav = useNav()
  const row = (label: string, value: ReactNode) => (
    <div key={label} className="flex min-h-[26px] items-baseline gap-4">
      <span className="w-60 shrink-0 font-title text-[21px] leading-tight text-white">{label}</span>
      <span className="flex-1 text-[15px] text-cm-yellow">{value}</span>
    </div>
  )
  const competition = c.division?.name
  return (
    <div className="min-h-0 flex-1 overflow-auto px-4 pb-3">
      {finances}
      <div className="py-1.5 text-center font-title text-xl font-bold text-cm-yellow">Informação Geral</div>
      {row('País', c.nationId !== null && c.nation ? <Link onClick={() => nav.go({ kind: 'nation', id: c.nationId! })}>{c.nation}</Link> : '-')}
      {row('Estatuto', `${c.status ?? '-'}${competition ? ` (${competition})` : ''}`)}
      {row('Finanças', `${financesLabel(c.cash)} · saldo ${money(c.cash)}`)}
      {row('Estádio', [c.stadium, c.stadiumCity].filter(Boolean).join(', ') || '-')}
      {row(
        'Lotação',
        c.stadiumCapacity
          ? `${c.stadiumCapacity.toLocaleString('pt-BR')}${c.stadiumSeated === c.stadiumCapacity ? ' - todos lugares sentados' : c.stadiumSeated ? ` (${c.stadiumSeated.toLocaleString('pt-BR')} sentados)` : ''}`
          : '-',
      )}
      {row('Reputação', c.reputation ?? '-')}
      {c.reserveDivision &&
        row('Estadual', <Link onClick={() => nav.go({ kind: 'competition', id: c.reserveDivision!.id })}>{c.reserveDivision.name}</Link>)}
      {row(
        'Rivais',
        c.rivals.length
          ? c.rivals.map((r, i) => (
              <span key={r.id}>
                {i > 0 && ', '}
                <Link onClick={() => nav.go({ kind: 'club', id: r.id })}>{r.shortName}</Link>
              </span>
            ))
          : '-',
      )}
      <div className="mb-1 mt-3 inline-block border-b-2 border-cm-orange font-title text-[20px] font-bold text-cm-orange">Staff</div>
      {c.staff.map((s) =>
        row(
          // "Sobrenome, Nome" -> "Nome Sobrenome", como na lista do CM
          s.listName.includes(', ') ? s.listName.split(', ').reverse().join(' ') : s.listName,
          <Link onClick={() => nav.go({ kind: 'person', id: s.id })}>{s.job ?? '-'}</Link>,
        ),
      )}
    </div>
  )
}

export default function ClubPage({ id, reserves }: { id: number; reserves?: boolean }) {
  const nav = useNav()
  const game = useGame()
  const [club, setClub] = useState<ClubProfile | null>(null)
  const [fixtures, setFixtures] = useState<Fixture[]>([])
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState('squad')
  const [view, setView] = useState<View>(reserves ? 'reserves' : 'players')
  const [criterion, setCriterion] = useState<Criterion>('position')
  const [selected, setSelected] = useState(0)
  const [tactics, setTactics] = useState<ClubTactics | null>(null)
  const gridRef = useRef<HTMLDivElement>(null)
  const mine = game.mode === 'game' && game.info?.club.id === id

  useEffect(() => {
    api.getClub(id).then(setClub, (e) => setError(String(e)))
  }, [id])

  // Jogos do clube e a tabela da competição, recarregados a cada dia do jogo
  const date = game.info?.date
  useEffect(() => {
    if (!date) return
    api.clubFixtures(id).then(
      (fs) => {
        setFixtures(fs)
      },
      () => {},
    )
  }, [id, date])
  // No clube do treinador, a escolha da equipa (para as casas GR, DD... SB1 do plantel)
  useEffect(() => {
    if (mine) api.getClubTactics(id).then(setTactics, () => {})
  }, [id, mine, date])

  const drag = useDragSwap({
    enabled: mine && !!tactics,
    onClick: () => {},
    onDrop: async (from, to) => {
      if (!tactics) return
      try {
        setTactics(await api.setTactics(swap(selectionOf(tactics), from, to)))
        const info = await api.gameInfo()
        if (info) game.update(info)
      } catch (e) {
        setError(String(e))
      }
    },
  })

  // "Limpar Equipa", como no CM: tira todos da equipa (antes do jogo o adjunto preenche o que ficar vazio)
  const clearTeam = async () => {
    if (!tactics) return
    try {
      const sel = selectionOf(tactics)
      setTactics(await api.setTactics({ ...sel, auto: false, starters: sel.starters.map(() => null), bench: [] }))
      const info = await api.gameInfo()
      if (info) game.update(info)
      setError(null)
    } catch (err) {
      setError(String(err))
    }
  }

  const toggle = async (e: Entry) => {
    if (!tactics) return
    try {
      const next = await api.togglePlayer(selectionOf(tactics), e.id)
      setTactics(await api.setTactics(selectionOf(next)))
      const info = await api.gameInfo()
      if (info) game.update(info)
      setError(null)
    } catch (err) {
      setError(String(err))
    }
  }

  const nextMatch = fixtures.find((f) => f.homeGoals === null)
  const lastMatch = fixtures.filter((f) => f.homeGoals !== null).at(-1)

  const entries: Entry[] = useMemo(() => {
    if (!club) return []
    if (view === 'staff') {
      return club.staff.map((s) => ({ id: s.id, number: '', badge: '', name: shortListName(s.listName), detail: s.job ?? '' }))
    }
    // A equipa de reservas (só no clube do treinador) fica numa vista à parte
    const players = club.players.filter((p) => p.reserve === (view === 'reserves'))
    const name = (a: { listName: string }, b: { listName: string }) => a.listName.localeCompare(b.listName)
    if (criterion === 'position') players.sort((a, b) => a.positionOrder - b.positionOrder || name(a, b))
    if (criterion === 'name') players.sort(name)
    if (criterion === 'number') players.sort((a, b) => byNullsLast(a.squadNumber, b.squadNumber) || name(a, b))
    if (criterion === 'age') players.sort((a, b) => byNullsLast(a.age, b.age) || name(a, b))
    if (criterion === 'ability') players.sort((a, b) => a.abilityRank - b.abilityRank)
    const sel = tactics && selectionOf(tactics)
    return players.map((p) => {
      const spot = sel ? spotOf(sel, p.id) : undefined
      // Casa na equipa (GR, DD... e SB1–SB7 nos suplentes); senão Les/Sus/Int como no CM
      const place =
        spot?.kind === 'starter' ? shortRole(tactics!.starters[spot.index].role) : spot?.kind === 'bench' ? `SB${spot.index + 1}` : null
      return {
      id: p.id,
      spot,
      number: criterion === 'age' ? (p.age ?? '') : (p.squadNumber ?? ''),
      // Num jogo em curso, os indisponíveis aparecem como no CM: "Les" / "Sus"
      badge: p.injured ? (
        <span className="bg-[#b8560c] px-0.5 font-bold text-white">Les</span>
      ) : p.suspended ? (
        <span className="bg-[#9c0000] px-0.5 font-bold text-white">Sus</span>
      ) : (
        (place ?? '')
      ),
      name: shortListName(p.listName),
      detail: p.position,
      }
    })
  }, [club, view, criterion, tactics])

  const open = (e: Entry | undefined) => e && nav.go({ kind: 'person', id: e.id })

  // Teclado: setas andam na grade de 2 colunas, Enter abre, Esc/Backspace voltam
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey) return
      const step = { ArrowUp: -2, ArrowDown: 2, ArrowLeft: -1, ArrowRight: 1 }[e.key]
      if (step !== undefined && tab === 'squad') {
        e.preventDefault()
        setSelected((s) => {
          const next = Math.max(0, Math.min(entries.length - 1, s + step))
          gridRef.current?.querySelector(`[data-row="${next}"]`)?.scrollIntoView({ block: 'nearest' })
          return next
        })
      } else if (e.key === 'Enter' && tab === 'squad') {
        open(entries[selected])
      } else if (e.key === 'Escape' || e.key === 'Backspace') {
        nav.back()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  const division = club?.division

  // Lugar de treinador vago: "Concorrer ao Lugar", como no CM
  const [vacancy, setVacancy] = useState<{ club: number; text: string | null } | null>(null)
  const nationId = club?.nationId ?? null
  const ownClub = club?.club.id === game.info?.club.id
  useEffect(() => {
    if (game.mode !== 'game' || ownClub || nationId === null) return
    api.jobsView(nationId).then(
      (j) => {
        const row = j.rows.find((r) => r.club.id === id)
        setVacancy(row?.vacant && !row.applied ? { club: id, text: null } : null)
      },
      () => setVacancy(null),
    )
  }, [game.mode, ownClub, nationId, id])
  const apply = async () => {
    try {
      const text = await api.applyJob(id)
      setVacancy({ club: id, text })
    } catch (e) {
      setVacancy({ club: id, text: String(e) })
    }
  }

  return (
    <div className="relative flex h-full flex-col">
      <ScreenHeader
        club={club?.club}
        eyebrow={
          club && (
            <>
              <span className="text-cm-orange">{club.division?.name ?? club.status ?? ''}</span>
              {view === 'reserves' && <span className="rounded bg-[#0f3a2a] px-1.5 text-[#4edea3]">Reservas</span>}
            </>
          )
        }
        title={club?.club.name ?? ''}
        right={
          <>
            {club?.stadium && (
              <span className="hidden rounded-md bg-black/30 px-3 py-1.5 font-mono text-[12px] text-[#86869a] xl:inline">
                Estádio: <b className="text-white">{club.stadium}</b>
                {club.stadiumCapacity ? (
                  <>
                    {' '}
                    · Lotação: <b className="text-white">{club.stadiumCapacity.toLocaleString('pt-BR')}</b>
                  </>
                ) : null}
              </span>
            )}
            {vacancy?.club === id &&
              (vacancy.text === null ? (
                <button className="h-8 rounded bg-cm-orange px-4 font-title text-[15px] font-bold text-black hover:brightness-110" onClick={() => void apply()}>
                  Concorrer ao Lugar
                </button>
              ) : (
                <span className="rounded bg-black/60 px-2 py-1 text-[13px] text-cm-yellow">{vacancy.text}</span>
              ))}
          </>
        }
      />
      <Tabs
        tabs={TABS.map((t) =>
          t.key === 'next' || t.key === 'fixtures' ? { ...t, disabled: fixtures.length === 0 } : t.key === 'transfers' ? { ...t, disabled: game.mode !== 'game' } : t,
        )}
        active={tab}
        onChange={setTab}
      />

      {tab === 'squad' && (
        <div className="mx-2.5 mt-2.5 flex shrink-0 items-center gap-1.5 rounded-md bg-[#3d1a73] px-2 py-1.5">
          <PurpleMenu
            label="Vêr"
            options={mine ? VIEWS : VIEWS.filter((v) => v.key !== 'reserves')}
            value={view}
            onChange={(v) => {
              setView(v)
              setSelected(0)
            }}
          />
          <PurpleMenu
            label="Critério"
            options={CRITERIA}
            value={criterion}
            disabled={view === 'staff'}
            onChange={(c) => {
              setCriterion(c)
              setSelected(0)
            }}
          />
          <div className="flex-1" />
          {mine && (
            <button className="cm-purple h-6 w-40" disabled={!tactics || view !== 'players'} onClick={() => void clearTeam()}>
              Limpar Equipa
            </button>
          )}
          {mine && (
            <button
              className="cm-purple h-6 w-40"
              disabled={!game.info?.numbersOpen}
              title={game.info?.numbersOpen ? undefined : 'Os números só se mudam na pré-época'}
              onClick={() => nav.go({ kind: 'squadNumbers' })}
            >
              Números
            </button>
          )}
          <PurpleMenu label="Filtrar" options={[]} disabled />
        </div>
      )}

      <div className="mx-2.5 mt-2.5 flex min-h-0 flex-1 flex-col">
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {club && tab === 'squad' && (
          <>
            {/* Casas da equipa: arraste um jogador para uma casa (ou para outro jogador) para mudar o lugar */}
            <div className="mb-2.5 flex shrink-0 flex-wrap items-center gap-0.5 rounded-md bg-[#1a1b21] px-3 py-1.5">
              <span className="mr-2 font-mono text-[12px] font-bold uppercase tracking-[0.12em] text-cm-orange">
                {view === 'staff' ? 'Staff' : CRITERIA.find((c) => c.key === criterion)!.label}
              </span>
              {mine && tactics && view === 'players' && (
              <>
                {tactics.starters.map((s, i) => {
                  const b = drag.bind({ kind: 'starter', index: i }, shortRole(s.role))
                  return (
                    <span key={`s${i}`} {...b} title={s.player?.listName ?? 'vazio'} className="cm-chip min-w-[38px] cursor-grab px-1 text-center text-[12px]">
                      {shortRole(s.role)}
                    </span>
                  )
                })}
                {/* Banco fixo de 9, como no CM: arraste um jogador para uma casa SB vazia para o pôr no banco */}
                {Array.from({ length: BENCH }, (_, i) => {
                  const p = tactics.bench[i]
                  const b = drag.bind({ kind: 'bench', index: i }, `SB${i + 1}`)
                  return (
                    <span
                      key={`b${i}`}
                      {...b}
                      title={p?.listName ?? 'vazio'}
                      className={`min-w-[38px] cursor-grab px-1 text-center text-[12px] ${p ? 'cm-chip' : 'cm-chip !bg-[#2a1a45] !text-[#8a63c4]'}`}
                    >
                      SB{i + 1}
                    </span>
                  )
                })}
                <span className="ml-2 self-center text-[11px] text-[#86869a]">Arraste para trocar · clique no número ou na casa para pôr/tirar</span>
              </>
              )}
            </div>
            <div ref={gridRef} className="min-h-0 flex-1 overflow-auto">
              {entries.length ? (
                <SquadGrid
                  entries={entries}
                  selected={selected}
                  onSelect={setSelected}
                  onOpen={open}
                  bind={mine && tactics ? drag.bind : undefined}
                  onToggle={mine && tactics && view === 'players' ? (e) => void toggle(e) : undefined}
                />
              ) : (
                <Empty>Ninguém registado.</Empty>
              )}
            </div>
          </>
        )}
        {tab !== 'squad' && (
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md bg-[#1a1b21]">
            {club && tab === 'info' && <Info c={club} finances={mine ? <FinancesPanel clubId={id} /> : undefined} />}
            {tab === 'next' && (
              <MatchInfo key="next" clubId={id} fixtures={fixtures} start={nextMatch ? fixtures.indexOf(nextMatch) : fixtures.length} empty="Não há mais jogos marcados." />
            )}
            {tab === 'last' && (
              <MatchInfo key="last" clubId={id} fixtures={fixtures} start={lastMatch ? fixtures.indexOf(lastMatch) : -1} empty="Ainda não jogou." />
            )}
            {tab === 'transfers' && <TransfersPanel clubId={id} />}
            {tab === 'fixtures' && <ClubFixtureList clubId={id} fixtures={fixtures} />}
          </div>
        )}
      </div>

      {/* Atalhos inferiores do CM; o Treino chega no marco 3.3 */}
      <div className="mt-2.5">
        <Tabs
          tabs={[
            { key: 'tactics', label: 'Tácticas' },
            { key: 'training', label: 'Treino', disabled: true },
            { key: 'last', label: 'Último Jogo', disabled: !lastMatch },
            { key: 'division', label: division?.shortName ?? '-', disabled: !division },
            { key: 'history', label: 'Historial' },
          ]}
          active={tab === 'last' ? 'last' : ''}
          onChange={(k) => {
            if (k === 'last') setTab('last')
            if (k === 'division' && division) nav.go({ kind: 'competition', id: division.id })
            if (k === 'tactics') nav.go({ kind: 'tactics', id })
            if (k === 'history') nav.go({ kind: 'clubHistory', id })
          }}
        />
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
      {drag.ghost}
    </div>
  )
}
