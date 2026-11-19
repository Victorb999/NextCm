import type { EditorHit } from '../../bindings/EditorHit'
import { COLUMNS, nextSort, type Kind, type Sort } from './editorList'

/** A lista detalhada dos resultados (como a do CM Scout): clicar num cabeçalho ordena por ele,
 * clicar numa linha abre-a. As linhas com mudanças por salvar levam um asterisco. */
export default function ResultsTable({
  kind,
  hits,
  sort,
  selected,
  dirty,
  onSort,
  onOpen,
}: {
  kind: Kind
  hits: EditorHit[]
  sort: Sort
  selected: number | null
  dirty: (id: number) => boolean
  onSort: (s: Sort) => void
  onOpen: (id: number) => void
}) {
  const columns = COLUMNS[kind]
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded border border-white/15 bg-black/30">
      <div className="min-h-0 flex-1 overflow-auto">
        <div className="w-max min-w-full">
          <div className="sticky top-0 z-10 flex h-7 shrink-0 items-center gap-2 border-b border-white/15 bg-[#0b2a1c] px-2">
            {columns.map((c) => {
              const on = sort.key === c.key
              return (
                <button
                  key={c.key}
                  onClick={() => onSort(nextSort(sort, c))}
                  title={`Ordenar por ${c.label}`}
                  className={`${c.width} shrink-0 truncate text-[12px] font-bold hover:text-white ${c.num ? 'text-right' : 'text-left'} ${
                    on ? 'text-cm-yellow' : 'text-cm-lilac'
                  }`}
                >
                  {c.label}
                  {on && (sort.desc ? ' ▾' : ' ▴')}
                </button>
              )
            })}
          </div>
          <div className="cm-zebra">
            {hits.map((h) => (
              <button
                key={h.id}
                onClick={() => onOpen(h.id)}
                className={`flex h-6 w-full items-center gap-2 px-2 text-left text-[14px] ${
                  selected === h.id ? 'bg-cm-sel text-white' : 'text-cm-yellow hover:bg-white/10'
                }`}
              >
                {columns.map((c) => (
                  <span
                    key={c.key}
                    className={`${c.width} shrink-0 truncate ${c.num ? 'text-right font-mono text-[12.5px]' : ''} ${
                      c.key === 'name' ? '' : 'text-white/85'
                    }`}
                  >
                    {c.key === 'name' && dirty(h.id) && <span className="text-cm-orange">* </span>}
                    {c.show(h)}
                  </span>
                ))}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
