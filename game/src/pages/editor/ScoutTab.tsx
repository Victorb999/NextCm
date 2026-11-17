import { useMemo, useState } from 'react'
import { api } from '../../api'
import type { EditorFilter } from '../../bindings/EditorFilter'
import type { EditorHit } from '../../bindings/EditorHit'
import type { EditorOptions } from '../../bindings/EditorOptions'
import { ErrorBox } from '../../components/Screen'
import { EMPTY_FILTER } from './editorEdits'
import { DEFAULT_SORT, sortHits, type Sort } from './editorList'
import ResultsTable from './ResultsTable'
import { TAB } from './editorText'
import SearchForm from './SearchForm'

/** A aba Busca do editor, como o CM Scout: só procura e mostra (os filtros e a lista ordenável),
 * sem editar nada. */
export default function ScoutTab({ file, options }: { file: string; options: EditorOptions | null }) {
  const [page, setPage] = useState<'filters' | 'list'>('filters')
  const [filter, setFilter] = useState<EditorFilter>(EMPTY_FILTER)
  const [hits, setHits] = useState<EditorHit[] | null>(null)
  const [sort, setSort] = useState<Sort>(DEFAULT_SORT.players)
  const [selected, setSelected] = useState<number | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const sorted = useMemo(() => sortHits(hits ?? [], sort), [hits, sort])

  const search = async () => {
    setBusy(true)
    setError(null)
    try {
      setNotice(null)
      setHits(await api.editorSearch(file, filter))
      setSelected(null)
      setPage('list')
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  /** Exporta a lista para CSV, na ordem em que está, ao lado dos saves. */
  const exportCsv = async () => {
    setBusy(true)
    setError(null)
    setNotice(null)
    try {
      setNotice(
        `Exportado para ${await api.editorExportCsv(
          file,
          sorted.map((h) => h.id),
        )}`,
      )
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <ErrorBox>{error}</ErrorBox>
      <div className="flex shrink-0 items-center gap-1 border-b border-white/15 pb-1">
        <button className={TAB(page === 'filters')} onClick={() => setPage('filters')}>
          Filtros
        </button>
        <button className={TAB(page === 'list')} onClick={() => setPage('list')}>
          Jogadores{hits ? ` (${hits.length})` : ''}
        </button>
        {busy && <span className="ml-auto pr-1 text-[13px] text-cm-lilac">Procurando…</span>}
      </div>
      {page === 'filters' ? (
        <div className="min-h-0 flex-1 overflow-y-auto">
          <SearchForm kind="players" filter={filter} options={options} busy={busy} onChange={setFilter} onSearch={() => void search()} />
        </div>
      ) : !hits ? (
        <div className="p-8 text-center text-cm-lilac">Escolha os filtros e clique em Procurar.</div>
      ) : (
        <>
          <div className="flex shrink-0 items-baseline gap-2 text-[13px]">
            <span className="text-cm-yellow">
              {hits.length} {hits.length === 1 ? 'jogador encontrado' : 'jogadores encontrados'}.
            </span>
            {hits.length >= 500 && <span className="text-cm-lilac">Mostrando os 500 melhores. Refine os filtros para ver outros.</span>}
            <span className="ml-auto text-cm-lilac">Clique num cabeçalho para ordenar.</span>
            <button className="cm-purple h-7 px-3 text-[13px]" disabled={!hits.length || busy} onClick={() => void exportCsv()}>
              Exportar CSV
            </button>
          </div>
          {notice && <div className="shrink-0 text-[13px] text-cm-lilac">{notice}</div>}
          {hits.length ? (
            <ResultsTable
              kind="players"
              hits={sorted}
              sort={sort}
              selected={selected}
              dirty={() => false}
              onSort={setSort}
              onOpen={setSelected}
            />
          ) : (
            <div className="p-8 text-center text-cm-lilac">Nenhum jogador com esses filtros. Volte a Filtros e afrouxe a busca.</div>
          )}
        </>
      )}
    </div>
  )
}
