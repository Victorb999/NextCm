import type { EditorFilter } from '../../bindings/EditorFilter'
import type { EditorOptions } from '../../bindings/EditorOptions'
import { EMPTY_FILTER } from './editorEdits'
import { POSITION_LABELS } from './editorText'

type Range = [keyof EditorFilter, keyof EditorFilter, string, number]
const RANGES: Range[] = [
  ['minAge', 'maxAge', 'Idade', 99],
  ['minAbility', 'maxAbility', 'Capacidade (CA)', 200],
  ['minPotential', 'maxPotential', 'Potencial (PA)', 200],
  ['minReputation', 'maxReputation', 'Reputação atual', 10000],
]

/** Os filtros da procura de jogadores, como no Player Search do editor do CM (o core filtra). */
export default function FilterBox({
  filter,
  options,
  onChange,
}: {
  filter: EditorFilter
  options: EditorOptions | null
  onChange: (f: EditorFilter) => void
}) {
  const set = (patch: Partial<EditorFilter>) => onChange({ ...filter, ...patch })
  const num = (v: string) => (v.trim() === '' ? null : Math.max(0, Number(v) || 0))
  const box = 'cm-field h-6 w-16 text-center text-[13px]'
  const select = (
    label: string,
    value: number | string | null,
    list: { id: number | string; name: string }[],
    change: (v: string) => void,
  ) => (
    <label className="flex h-6 items-center gap-2 text-[13px] text-white">
      <span className="w-20 shrink-0">{label}</span>
      <select className="cm-field h-6 min-w-0 flex-1 text-[13px]" value={value ?? ''} onChange={(e) => change(e.target.value)}>
        <option value="">Todas</option>
        {list.map((o) => (
          <option key={o.id} value={o.id}>
            {o.name}
          </option>
        ))}
      </select>
    </label>
  )
  return (
    <div className="flex flex-col gap-1 rounded border border-white/15 bg-black/20 p-1.5">
      {RANGES.map(([min, max, label, top]) => (
        <div key={label} className="flex h-6 items-center gap-1 text-[13px] text-white">
          <span className="min-w-0 flex-1 truncate">{label}</span>
          <input
            className={box}
            inputMode="numeric"
            placeholder="0"
            value={(filter[min] as number | null) ?? ''}
            onChange={(e) => set({ [min]: num(e.target.value) })}
          />
          <span>–</span>
          <input
            className={box}
            inputMode="numeric"
            placeholder={String(top)}
            value={(filter[max] as number | null) ?? ''}
            onChange={(e) => set({ [max]: num(e.target.value) })}
          />
        </div>
      ))}
      {select(
        'Posição',
        filter.positions[0] ?? null,
        (options?.positionKeys ?? []).map((k) => ({ id: k, name: POSITION_LABELS[k] ?? k })),
        (v) => set({ positions: v ? [v] : [] }),
      )}
      {select('Nação', filter.nationId, options?.nations ?? [], (v) => set({ nationId: v ? Number(v) : null }))}
      {select('Joga em', filter.basedId, options?.nations ?? [], (v) => set({ basedId: v ? Number(v) : null }))}
      {select('Divisão', filter.divisionId, options?.divisions ?? [], (v) => set({ divisionId: v ? Number(v) : null }))}
      <button type="button" className="cm-purple h-6 text-[12.5px]" onClick={() => onChange({ ...EMPTY_FILTER, query: filter.query })}>
        Limpar filtros
      </button>
    </div>
  )
}
