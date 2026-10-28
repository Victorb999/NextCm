import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { NumberAssign } from '../bindings/NumberAssign'
import type { SquadNumbers } from '../bindings/SquadNumbers'
import { BottomBar, PurpleMenu, ScreenHeader } from '../components/Screen'
import { initials, shortListName } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// "Números Plantel", como no CM: escolher um jogador e clicar num número livre da grelha
// (só na pré-época ou para quem não tem número: a regra está no core).
// Auto dá números a quem não tem (pelas preferências, no core); Anular volta ao gravado;
// Enviar grava.

type View = 'missing' | 'all'
type Criterion = 'position' | 'name' | 'number'

const VIEWS: { key: View; label: string }[] = [
  { key: 'missing', label: 'Sem número' },
  { key: 'all', label: 'Todos' },
]
const CRITERIA: { key: Criterion; label: string }[] = [
  { key: 'position', label: 'Posição' },
  { key: 'name', label: 'Nome' },
  { key: 'number', label: 'Número' },
]

/** A grelha de 1 a 99: os livres claros, os ocupados apagados (com o apelido de quem o tem), o escolhido amarelo. */
function NumberGrid({
  owners,
  current,
  disabled,
  onPick,
}: {
  /** Número → apelido de quem o tem. */
  owners: Map<number, string>
  current: number | null
  disabled: boolean
  onPick: (n: number) => void
}) {
  return (
    // Ocupa o espaço que houver: em janelas baixas só o número, nas altas também o apelido
    <div className="grid min-h-0 flex-1 grid-cols-10 grid-rows-10 gap-1">

      {Array.from({ length: 99 }, (_, i) => i + 1).map((n) => {
        const owner = owners.get(n)
        const mine = n === current
        return (
          <button
            key={n}
            disabled={disabled || (owner !== undefined && !mine)}
            onClick={() => onPick(n)}
            title={owner}
            className={`flex min-h-0 flex-col items-center justify-center rounded-[4px] leading-tight tabular-nums ${
              mine
                ? 'bg-cm-yellow text-black ring-2 ring-cm-yellow/40'
                : owner !== undefined
                  ? 'bg-[#16171c] text-[#55565f]'
                  : 'bg-[#2b2b30] text-white hover:bg-cm-sel disabled:hover:bg-[#2b2b30]'
            }`}
          >
            <span className="text-[15px] font-bold">{n}</span>
            <span className={`hidden max-w-full truncate px-1 text-[9px] uppercase tracking-wide [@media(min-height:860px)]:block ${mine ? 'font-bold' : owner !== undefined ? '' : 'text-[#4edea3]'}`}>
              {mine ? 'Escolha' : (owner ?? 'Livre')}
            </span>
          </button>
        )
      })}
    </div>
  )
}

/** A casa do número na lista: vazia e tracejada quando ainda não tem. */
function NumberBox({ n }: { n: number | null }) {
  return n === null ? (
    <span className="flex h-[22px] w-9 shrink-0 items-center justify-center rounded-[3px] border border-dashed border-[#55565f] text-[11px] text-[#86869a]">
      -
    </span>
  ) : (
    <span className="flex h-[22px] w-9 shrink-0 items-center justify-center rounded-[3px] bg-[#2b2b30] text-[13px] font-bold tabular-nums text-white">{n}</span>
  )
}

export default function SquadNumbersPage() {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [saved, setSaved] = useState<SquadNumbers | null>(null)
  const [numbers, setNumbers] = useState<Map<number, number | null>>(new Map())
  const [selected, setSelected] = useState<number | null>(null)
  const [view, setView] = useState<View>('all')
  const [criterion, setCriterion] = useState<Criterion>('position')
  const [error, setError] = useState<string | null>(null)

  const reset = (s: SquadNumbers) => {
    setSaved(s)
    setNumbers(new Map(s.players.map((p) => [p.id, p.number])))
    // Começa no primeiro sem número
    setSelected(s.players.find((p) => p.number === null)?.id ?? null)
    if (s.players.some((p) => p.number === null)) setView('missing')
  }
  useEffect(() => {
    api.squadNumbers().then(reset, (e) => setError(String(e)))
  }, [])

  const players = useMemo(() => {
    if (!saved) return []
    const list = saved.players.filter((p) => view === 'all' || numbers.get(p.id) === null || p.id === selected)
    const num = (id: number) => numbers.get(id) ?? 999
    if (criterion === 'name') list.sort((a, b) => a.listName.localeCompare(b.listName))
    if (criterion === 'number') list.sort((a, b) => num(a.id) - num(b.id))
    if (criterion === 'position') list.sort((a, b) => a.positionOrder - b.positionOrder || a.listName.localeCompare(b.listName))
    return list
  }, [saved, numbers, view, criterion, selected])

  const current = selected !== null ? (numbers.get(selected) ?? null) : null
  /** Fora da pré-época quem já tinha número fica com ele (regra no core). */
  const locked = (id: number) => !!saved && !saved.preSeason && saved.players.some((p) => p.id === id && p.number !== null)
  const pick = (n: number) => {
    if (selected === null || locked(selected)) return
    const next = new Map(numbers)
    next.set(selected, n === current ? null : n)
    setNumbers(next)
    // Passa ao seguinte sem número
    const after = saved?.players.find((p) => p.id !== selected && next.get(p.id) === null)
    if (n !== current && after) setSelected(after.id)
  }
  const assigns = (): NumberAssign[] => [...numbers.entries()].map(([id, number]) => ({ id, number }))
  const auto = async () => {
    try {
      const got = await api.autoSquadNumbers(assigns())
      setNumbers(new Map(got.map((a) => [a.id, a.number])))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }
  const send = async () => {
    try {
      reset(await api.setSquadNumbers(assigns()))
      const info = await api.gameInfo()
      if (info) game.update(info)
      nav.back()
    } catch (e) {
      setError(String(e))
    }
  }

  const missing = [...numbers.values()].filter((n) => n === null).length
  // Quem tem cada número (menos o escolhido, cujo número fica amarelo)
  const owners = new Map<number, string>()
  for (const p of saved?.players ?? []) {
    const n = numbers.get(p.id)
    if (n !== null && n !== undefined && p.id !== selected) owners.set(n, p.listName.split(', ')[0])
  }
  const chosen = saved?.players.find((p) => p.id === selected)
  const club = saved?.club

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        emblem={club && { text: initials(club.shortName), fg: club.fg ?? '#ffffff', bg: club.bg ?? '#3a3a3a' }}
        eyebrow={
          saved && (
            <span className={`rounded px-1.5 tracking-wider ${saved.preSeason ? 'bg-[#0f3a2a] text-[#4edea3]' : 'bg-[#3a2508] text-cm-orange'}`}>
              {saved.preSeason ? 'Pré-época' : 'Época a decorrer'}
            </span>
          )
        }
        title={club ? `Números Plantel ${club.shortName}` : 'Números Plantel'}
      />
      {saved && (
        <div className="mx-2.5 mt-2.5 flex shrink-0 items-center gap-3 rounded-md border border-cm-yellow/60 bg-cm-yellow/[0.06] px-4 py-2 text-[13px] text-[#e3e1e9]">
          <span className="font-bold text-cm-yellow">i</span>
          {saved.preSeason ? 'Os números ficam fixos a partir do primeiro jogo oficial.' : 'Com a época a decorrer, só os jogadores sem número recebem um.'}
          <span className="text-[#55565f]">·</span>
          <span>
            Total no plantel: <b>{saved.players.length} jogadores</b>
          </span>
          {missing > 0 && <span className="ml-auto rounded bg-cm-sel px-2 py-0.5 text-[12px] font-bold text-white">Sem número: {missing}</span>}
        </div>
      )}
      <div className="mx-2.5 mt-2.5 flex shrink-0 items-center gap-1.5">
        <PurpleMenu label="Vêr" options={VIEWS} value={view} onChange={setView} />
        <PurpleMenu label="Critério" options={CRITERIA} value={criterion} onChange={setCriterion} />
        <div className="flex-1" />
        <button className="cm-purple h-7 w-32" onClick={() => void auto()} title="Números para quem não tem, pelas preferências">
          Auto
        </button>
        <button className="cm-purple h-7 w-32" onClick={() => saved && reset(saved)} title="Volta aos números gravados">
          Anular
        </button>
      </div>
      {error && <div className="mx-2.5 mt-2.5 rounded bg-red-900 px-2 py-1 text-white">{error}</div>}
      <div className="mx-2.5 mt-2.5 flex min-h-0 flex-1 gap-2.5">
        <div className="flex min-h-0 w-[46%] flex-col overflow-hidden rounded-md bg-[#1a1b21]">
          <div className="flex h-8 shrink-0 items-center gap-3 border-b border-cm-line bg-black/40 px-4 text-[11px] font-bold uppercase tracking-[0.12em] text-[#86869a]">
            <span className="w-9 text-center">Nº</span>
            <span className="flex-1">Jogador</span>
            <span>Posição</span>
          </div>
          <div className="cm-zebra min-h-0 flex-1 overflow-auto">
            {players.length === 0 && <div className="p-4 text-center text-cm-magenta">Todos os jogadores têm número.</div>}
            {players.map((p) => {
              const sel = p.id === selected
              return (
                <div
                  key={p.id}
                  onClick={() => !locked(p.id) && setSelected(p.id)}
                  title={locked(p.id) ? 'Com a época a decorrer, este número está fixo' : undefined}
                  className={`flex h-[30px] items-center gap-3 px-4 text-[14.5px] ${sel ? '!bg-cm-sel' : ''} ${
                    locked(p.id) ? 'opacity-45' : 'cursor-pointer hover:bg-white/[0.06]'
                  }`}
                >
                  <NumberBox n={numbers.get(p.id) ?? null} />
                  <span className="min-w-0 flex-1 truncate text-white">
                    {shortListName(p.listName)}
                    {sel && <span className="ml-2 rounded-[3px] bg-cm-yellow px-1 text-[10px] font-bold uppercase text-black">Seleccionado</span>}
                  </span>
                  <span className={`text-[13px] font-semibold ${sel ? 'text-white' : 'text-cm-orange'}`}>{p.position}</span>
                </div>
              )
            })}
          </div>
          <div className="flex h-8 shrink-0 items-center justify-between border-t border-cm-line px-4 text-[11px] text-[#86869a]">
            <span>Clique num jogador para escolher ou trocar o número</span>
            <span>{players.length} na lista</span>
          </div>
        </div>
        <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-3 rounded-md bg-[#1a1b21] p-4">
          <div className="flex shrink-0 items-center gap-3">
            <div>
              <div className="text-[11px] font-bold uppercase tracking-[0.12em] text-[#86869a]">Painel de escolha</div>
              <div className="font-title text-[20px] font-bold text-white">Escolha o número</div>
            </div>
            {chosen && (
              <div className="ml-auto flex items-center gap-3 rounded-md bg-cm-sel/80 px-3 py-1.5">
                <span className="leading-tight">
                  <span className="block font-bold text-white">{shortListName(chosen.listName)}</span>
                  <span className="text-[12px] text-cm-orange">{chosen.position}</span>
                </span>
                <span className="rounded bg-cm-yellow px-2 py-0.5 text-[12px] font-bold text-black">{current ?? 'A atribuir'}</span>
              </div>
            )}
          </div>
          <NumberGrid owners={owners} current={current} disabled={!chosen || locked(chosen.id)} onPick={pick} />
          <div className="flex shrink-0 items-center gap-4 text-[12px] text-[#c8c8d0]">
            <span className="flex items-center gap-1.5">
              <span className="size-3 rounded-[2px] bg-[#2b2b30]" /> Livre
            </span>
            <span className="flex items-center gap-1.5">
              <span className="size-3 rounded-[2px] bg-[#16171c] ring-1 ring-[#2c2c33]" /> Ocupado
            </span>
            <span className="flex items-center gap-1.5">
              <span className="size-3 rounded-[2px] bg-cm-yellow" /> Escolhido
            </span>
            {chosen && current !== null && (
              <span className="ml-auto">
                Número escolhido: <b className="text-white">#{current}</b> para <b className="text-cm-yellow">{shortListName(chosen.listName)}</b>
              </span>
            )}
          </div>
        </div>
      </div>
      <BottomBar right="Enviar" onLeft={nav.back} onRight={saved ? () => void send() : undefined} />
    </div>
  )
}
