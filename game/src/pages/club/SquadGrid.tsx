import { HeaderCell, InfBadge, type HeaderProps } from '../../components/SquadTable'
import { Link } from '../../components/Screen'
import { type Spot } from '../../selection'
import { nameClass } from '../../squad'
import type { Entry, Bind } from './squadView'

export function SquadGrid({
  entries,
  selected,
  onSelect,
  onOpen,
  bind,
  onToggle,
  header,
  numbers,
}: {
  entries: Entry[]
  selected: number
  onSelect: (i: number) => void
  onOpen: (e: Entry) => void
  /** Só no clube do treinador: arrastar para mudar o lugar na equipa. */
  bind?: (spot: Spot, label: string) => Bind
  /** Só no clube do treinador: clicar no número ou na casa põe/tira da equipa. */
  onToggle?: (e: Entry) => void
  /** Os cabeçalhos que ordenam (fora do Staff). */
  header?: HeaderProps
  /** A coluna Nº (com o Critério Numeração ou Idade); o CM não a mostra. */
  numbers?: boolean
}) {
  const row = (e: Entry, i: number) => {
    const drag = bind && e.spot ? bind(e.spot, e.name) : undefined
    const toggle = onToggle && (() => onToggle(e))
    const stop = onToggle && ((ev: React.MouseEvent) => ev.stopPropagation())
    return (
      <div
        key={e.id}
        data-row={i}
        data-person={e.id}
        data-spot={drag?.['data-spot']}
        onMouseDown={drag?.onMouseDown}
        onClick={() => onSelect(i)}
        onDoubleClick={() => onOpen(e)}
        className={`flex h-6 items-center gap-1.5 px-2 ${i === selected ? '!bg-cm-sel' : ''} ${drag ? 'cursor-grab' : ''}`}
      >
        {numbers && (
          <span
            onMouseDown={stop}
            onClick={toggle}
            className={`w-8 shrink-0 text-center font-mono text-[13px] text-cm-muted ${onToggle ? 'cursor-pointer' : ''}`}
          >
            {e.number}
          </span>
        )}
        {/* A casa na equipa e a sigla Inf em caixas escuras, vazias quando não há, como no CM */}
        <span
          onMouseDown={stop}
          onClick={toggle}
          title={onToggle ? 'Clique para pôr/tirar da equipa' : undefined}
          className={`w-10 shrink-0 text-center font-mono text-[12px] 
            font-bold ${e.badge ? 'cm-chip' : 'h-[18px] rounded bg-black/50'} ${onToggle ? 'cursor-pointer' : ''}`}
        >
          {e.badge}
        </span>
        {e.player && (
          <span className="flex h-[18px] w-10 shrink-0 items-center justify-center rounded bg-black/50 font-mono text-[12px]">
            {e.player.info && <InfBadge info={e.player.info} person={e.player.id} />}
          </span>
        )}
        <span
          className={`min-w-0 flex-1 truncate text-[16px] font-semibold ${e.player ? nameClass(e.player) : 'text-white'}`}
          title={e.player?.ghost ? 'Jogador de enchimento' : e.player?.loaned ? 'Emprestado' : e.player?.free ? 'Sem contrato' : undefined}
        >
          <Link onClick={() => onOpen(e)}>{e.name}</Link>
        </span>
        <span className={`w-28 shrink-0 truncate text-left font-mono text-[12.5px] ${i === selected ? 'text-white' : 'text-cm-yellow'}`}>
          {e.detail}
        </span>
      </div>
    )
  }
  const head = (
    <div className="flex h-7 shrink-0 items-center gap-1.5 border-b border-cm-line bg-black/30 px-2 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-dim">
      {numbers && <HeaderCell label="Nº" className="w-8 text-center" header={header} />}
      <span className="w-10 text-center">Esc</span>
      {entries[0]?.player && <span className="w-10 text-center">Inf</span>}
      <HeaderCell label="Jogador" className="flex-1" header={header} />
      <HeaderCell label="Posição" className="w-28 text-left" header={header} />
    </div>
  )
  const column = (side: 0 | 1) => (
    <div className="min-w-0 flex-1 overflow-hidden rounded-md bg-cm-surface">
      {head}
      <div className="cm-zebra">{entries.map((e, i) => (i % 2 === side ? row(e, i) : null)).filter(Boolean)}</div>
    </div>
  )
  return (
    <div className="flex items-start gap-2">
      {column(0)}
      {column(1)}
    </div>
  )
}
