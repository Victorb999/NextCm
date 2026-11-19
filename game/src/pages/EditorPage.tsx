import { useCallback, useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { EditorAttribute } from '../bindings/EditorAttribute'
import type { EditorFilter } from '../bindings/EditorFilter'
import type { EditorHit } from '../bindings/EditorHit'
import type { EditorOptions } from '../bindings/EditorOptions'
import type { EditorProfile } from '../bindings/EditorProfile'
import type { SaveSummary } from '../bindings/SaveSummary'
import { DialogFrame, Overlay } from '../components/Dialog'
import Loading from '../components/Loading'
import { ErrorBox } from '../components/Screen'
import { useGame } from '../game'
import ClubDialog from './editor/ClubDialog'
import CommandDialog from './editor/CommandDialog'
import ContractTab from './editor/ContractTab'
import {
  clubDirty,
  collectChanges,
  dirtyNames,
  EMPTY_FILTER,
  NO_EDITS,
  playerDirty,
  revertAll,
  staffDirty,
  type ClubEdit,
  type Edits,
  type PlayerEdit,
  type StaffEdit,
} from './editor/editorEdits'
import { BOX, LABELS, POS_LEFT, POS_RIGHT, POSITION_LABELS, ROW, TAB } from './editor/editorText'
import { AttributeRow, DateField, Header, NationSelect, NumField } from './editor/Fields'
import FilterBox from './editor/FilterBox'
import PrefsTab from './editor/PrefsTab'
import ScoutTab from './editor/ScoutTab'
import StaffPanel from './editor/StaffPanel'

// Editor de jogos salvos (marco 3.20): procura jogadores, staff e clubes num save fechado e
// edita-os. Os atributos vão pelo valor de 1 a 20 que o jogo mostra; o core converte para o valor
// guardado (ao lado, só de leitura). As mudanças acumulam-se (de vários jogadores e clubes) e o
// "Gravar Alterações" grava tudo de uma vez.

type Kind = 'players' | 'staff' | 'clubs'
type Tab = 'data' | 'attr1' | 'attr2' | 'contract' | 'prefs'

/** "Editor de Jogos Salvos": no formato do editor do CM (lista à esquerda, abas à direita). */
export default function EditorPage() {
  const game = useGame()
  const [saves, setSaves] = useState<SaveSummary[]>([])
  const [file, setFile] = useState('')
  const [options, setOptions] = useState<EditorOptions | null>(null)
  const [kind, setKind] = useState<Kind>('players')
  const [tab, setTab] = useState<Tab>('data')
  const [filter, setFilter] = useState<EditorFilter>(EMPTY_FILTER)
  const [filtersOpen, setFiltersOpen] = useState(false)
  // Editor (a tela de sempre) ou Busca (só procurar, como o CM Scout)
  const [screen, setScreen] = useState<'editor' | 'scout'>('editor')
  const [hits, setHits] = useState<EditorHit[]>([])
  const [edits, setEdits] = useState<Edits>(NO_EDITS)
  const [selected, setSelected] = useState<number | null>(null)
  const [clubOpen, setClubOpen] = useState<number | null>(null)
  const [clubIds, setClubIds] = useState<number[]>([])
  const [commandOpen, setCommandOpen] = useState(false)
  // Sair, ou trocar de jogo, com mudanças por gravar: o que fazer depois de responder
  const [leaving, setLeaving] = useState<(() => void) | null>(null)
  // Copiar/Colar atributos: os valores mostrados de um jogador, para colar no rascunho de outro
  const [clip, setClip] = useState<{ name: string; values: Record<string, number> } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    api.listSaves().then(
      (list) => {
        setSaves(list)
        setFile((f) => f || list[0]?.file || '')
      },
      (e) => setError(String(e)),
    )
  }, [])

  useEffect(() => {
    if (!file) return
    let live = true
    api.editorOptions(file).then(
      (o) => live && setOptions(o),
      (e) => live && setError(String(e)),
    )
    return () => {
      live = false
    }
  }, [file])

  const pending = useMemo(() => dirtyNames(edits), [edits])
  const dirty = pending.length > 0

  // Fechar a janela (ou recarregar) com mudanças por gravar: o navegador pergunta
  useEffect(() => {
    if (!dirty) return
    const warn = (e: BeforeUnloadEvent) => e.preventDefault()
    window.addEventListener('beforeunload', warn)
    return () => window.removeEventListener('beforeunload', warn)
  }, [dirty])

  const run = useCallback(async <T,>(job: () => Promise<T>): Promise<T | undefined> => {
    setBusy(true)
    setError(null)
    try {
      return await job()
    } catch (e) {
      setError(String(e))
      return undefined
    } finally {
      setBusy(false)
    }
  }, [])

  const resetView = () => {
    setHits([])
    setClubOpen(null)
    setSelected(null)
    setNotice(null)
  }

  /** Faz `then` já, ou pergunta primeiro se há mudanças por gravar. */
  const guard = (then: () => void) => (dirty ? setLeaving(() => then) : then())

  const search = async () => {
    if (!file) return
    const found = await run(() =>
      kind === 'players'
        ? api.editorSearch(file, filter)
        : kind === 'staff'
          ? api.editorSearchStaff(file, filter.query)
          : api.editorSearchClubs(file, filter.query),
    )
    if (found) {
      setHits(found)
      setSelected(null)
      setNotice(found.length ? null : kind === 'clubs' ? 'Nenhum clube encontrado.' : 'Ninguém encontrado.')
    }
  }

  /** Lista todos os jogadores do clube e passa para o modo Jogadores. */
  const showSquad = async (clubId: number) => {
    const found = await run(() => api.editorSquad(file, clubId))
    if (found) {
      resetView()
      setKind('players')
      setHits(found)
      setNotice(found.length ? null : 'O clube não tem jogadores.')
    }
  }

  /** Carrega um jogador do save (a ficha e os atributos), sem o rascunho. */
  const loadPlayer = async (id: number): Promise<PlayerEdit | undefined> => {
    const both = await run(() => Promise.all([api.editorPlayer(file, id), api.editorProfile(file, id)]))
    return both && { player: both[0], view: both[1], draft: {}, profile: both[1].profile }
  }

  const open = async (id: number) => {
    setNotice(null)
    if (kind === 'clubs') {
      setSelected(id)
      setClubIds(hits.map((h) => h.id))
      setClubOpen(id)
      return
    }
    if (kind === 'staff' || edits.players[id]) {
      setSelected(id)
      return
    }
    const pe = await loadPlayer(id)
    if (!pe) return
    setEdits((e) => ({ ...e, players: { ...e.players, [id]: pe } }))
    setSelected(id)
  }

  /** Grava tudo o que está por gravar; depois, reabre o jogador aberto com os valores do save. */
  const saveAll = async (): Promise<boolean> => {
    const n = await run(() => api.editorSave(file, collectChanges(edits)))
    if (n === undefined) return false
    const reopen = kind === 'players' && selected !== null ? await loadPlayer(selected) : undefined
    setEdits(reopen && selected !== null ? { ...NO_EDITS, players: { [selected]: reopen } } : NO_EDITS)
    setNotice(`Salvo: ${n} alteraç${n === 1 ? 'ão' : 'ões'} no jogo.`)
    return true
  }

  const pe: PlayerEdit | undefined = kind === 'players' && selected !== null ? edits.players[selected] : undefined
  const setPE = (f: (e: PlayerEdit) => PlayerEdit) =>
    selected !== null &&
    setEdits((e) => {
      const cur = e.players[selected]
      return cur ? { ...e, players: { ...e.players, [selected]: f(cur) } } : e
    })
  const setP = (patch: Partial<EditorProfile>) => setPE((e) => ({ ...e, profile: { ...e.profile, ...patch } }))
  const setDraftValues = (values: Record<string, number>) => setPE((e) => ({ ...e, draft: { ...e.draft, ...values } }))
  const onClubEdit = useCallback((c: ClubEdit) => setEdits((e) => ({ ...e, clubs: { ...e.clubs, [c.saved.id]: c } })), [])
  const onStaffEdit = useCallback((s: StaffEdit) => setEdits((e) => ({ ...e, staff: { ...e.staff, [s.staff.id]: s } })), [])

  /** Max/Min/Igualar: aplica a todos os atributos do conjunto (Igualar = a média arredondada). */
  const setAll = (group: (a: EditorAttribute) => boolean, how: 'max' | 'min' | 'equal') => {
    if (!pe) return
    const list = pe.player.attributes.filter(group)
    const mean = Math.round(list.reduce((s, a) => s + (pe.draft[a.key] ?? a.shown), 0) / Math.max(1, list.length))
    const value = how === 'max' ? 20 : how === 'min' ? 1 : mean
    setDraftValues(Object.fromEntries(list.map((a) => [a.key, value])))
  }

  /** Copia os atributos do jogador aberto (com o que estiver por gravar). */
  const copyAttributes = () => {
    if (!pe) return
    setClip({ name: pe.player.name, values: Object.fromEntries(pe.player.attributes.map((a) => [a.key, pe.draft[a.key] ?? a.shown])) })
    setNotice(`Atributos de ${pe.player.name} copiados.`)
  }

  /** Cola no rascunho do jogador aberto: grava-se com o resto (e o core valida). */
  const pasteAttributes = () => {
    if (!pe || !clip) return
    setDraftValues(
      Object.fromEntries(pe.player.attributes.filter((a) => clip.values[a.key] !== undefined).map((a) => [a.key, clip.values[a.key]!])),
    )
    setNotice(`Atributos de ${clip.name} colados.`)
  }

  /** Exporta a lista da esquerda para CSV, ao lado dos saves (o que está gravado no jogo). */
  const exportCsv = async () => {
    const path = await run(() =>
      api.editorExportCsv(
        file,
        hits.map((h) => h.id),
      ),
    )
    if (path) setNotice(`Exportado para ${path}${dirty ? ' (sem as alterações não salvas)' : ''}`)
  }

  /** Corre o Comando Personalizado (gravando antes o que estiver por gravar) e reabre o jogador aberto. */
  const runCommand = async (commands: string[], firstTeam: boolean, onlyPerson: boolean) => {
    setCommandOpen(false)
    if (dirty && !(await saveAll())) return
    const ids = onlyPerson && pe ? [pe.player.id] : hits.map((h) => h.id)
    const n = await run(() => api.editorCommand(file, ids, commands, firstTeam))
    if (n === undefined) return
    const reopen = pe ? await loadPlayer(pe.player.id) : undefined
    setEdits(reopen ? { ...NO_EDITS, players: { [reopen.player.id]: reopen } } : NO_EDITS)
    setNotice(`Comando aplicado a ${n} jogador${n === 1 ? '' : 'es'}.`)
  }

  const p = pe?.profile
  const view = pe?.view
  const mentalOf = (a: EditorAttribute) => a.group === 'mental'
  const playingOf = (a: EditorAttribute) => a.group !== 'mental'
  // Os atributos de jogo, por ordem alfabética do rótulo, repartidos em três colunas como no print
  const playing = useMemo(
    () =>
      pe ? pe.player.attributes.filter(playingOf).sort((a, b) => (LABELS[a.key] ?? a.key).localeCompare(LABELS[b.key] ?? b.key, 'pt')) : [],
    [pe],
  )
  const per = Math.ceil(playing.length / 3)
  const columns = [playing.slice(0, per), playing.slice(per, per * 2), playing.slice(per * 2)]
  const attrRow = (a: EditorAttribute) => (
    <AttributeRow key={a.key} a={a} value={pe?.draft[a.key] ?? a.shown} onChange={(v) => setDraftValues({ [a.key]: v })} />
  )
  const positionField = (k: string) =>
    p &&
    view && (
      <NumField
        key={k}
        label={POSITION_LABELS[k] ?? k}
        value={p.positions[view.positionKeys.indexOf(k)] ?? 1}
        min={1}
        max={20}
        width="w-16"
        onChange={(v) =>
          setP({ positions: p.positions.map((x, j) => (j === view.positionKeys.indexOf(k) ? Math.min(20, Math.max(1, v)) : x)) })
        }
      />
    )
  /** Uma linha da lista com mudanças por gravar leva um asterisco. */
  const rowDirty = (id: number) => {
    const e = kind === 'players' ? edits.players[id] : kind === 'staff' ? edits.staff[id] : edits.clubs[id]
    if (!e) return false
    return kind === 'players' ? playerDirty(e as PlayerEdit) : kind === 'staff' ? staffDirty(e as StaffEdit) : clubDirty(e as ClubEdit)
  }

  return (
    <Overlay>
      <DialogFrame
        title="Editor de Jogos Salvos"
        full
        onClose={() => guard(() => game.setMode('start'))}
        header={
          saves.length > 0 && (
            <div className="mt-1.5 flex shrink-0 gap-1">
              <button className={TAB(screen === 'editor')} onClick={() => setScreen('editor')}>
                Editor
              </button>
              <button className={TAB(screen === 'scout')} onClick={() => setScreen('scout')}>
                Busca
              </button>
            </div>
          )
        }
      >
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {saves.length === 0 ? (
          <div className="p-6 text-center text-cm-yellow">Nenhum jogo salvo para editar.</div>
        ) : screen === 'scout' ? (
          <ScoutTab file={file} options={options} />
        ) : (
          <div className="flex h-full gap-3">
            <div className="flex min-h-0 w-80 shrink-0 flex-col gap-1.5 overflow-y-auto pr-1 [&>*]:shrink-0">
              <select
                className="cm-field h-7 text-[14px]"
                value={file}
                onChange={(e) => {
                  const next = e.target.value
                  guard(() => {
                    setEdits(NO_EDITS)
                    setFile(next)
                    resetView()
                  })
                }}
              >
                {saves.map((s) => (
                  <option key={s.file} value={s.file}>
                    {s.name} · {s.clubName}
                  </option>
                ))}
              </select>
              <div className="flex h-[300px] flex-auto flex-col overflow-hidden rounded border border-white/15 bg-black/30">
                <div className="flex h-6 shrink-0 items-center gap-2 border-b border-white/15 px-2 text-[12px] font-bold text-cm-lilac">
                  <span className="flex-1">Nome</span>
                  <span className="w-9 text-right">{kind === 'clubs' ? 'Rep.' : 'CA'}</span>
                  {kind !== 'clubs' && <span className="w-9 text-right">PA</span>}
                </div>
                <div className="cm-zebra min-h-0 flex-1 overflow-auto">
                  {hits.map((h) => (
                    <button
                      key={h.id}
                      onClick={() => void open(h.id)}
                      title={h.club ?? undefined}
                      className={`flex h-6 w-full items-center gap-2 px-2 text-left text-[14px] ${selected === h.id ? 'bg-cm-sel text-white' : 'text-cm-yellow'}`}
                    >
                      <span className="min-w-0 flex-1 truncate">
                        {rowDirty(h.id) && <span className="text-cm-orange">* </span>}
                        {h.name}
                      </span>
                      <span className="w-9 text-right font-mono text-[12px]">{h.ability ?? ''}</span>
                      {kind !== 'clubs' && <span className="w-9 text-right font-mono text-[12px]">{h.potential ?? ''}</span>}
                    </button>
                  ))}
                </div>
                {notice && selected === null && <div className="p-3 text-center text-[13px] text-cm-lilac">{notice}</div>}
              </div>
              <div className="text-[13px] text-cm-yellow">{hits.length} itens encontrados.</div>
              <form
                className="flex flex-col gap-1.5"
                onSubmit={(e) => {
                  e.preventDefault()
                  void search()
                }}
              >
                <input
                  className="cm-field h-7 text-[14px]"
                  placeholder={
                    kind === 'players' ? 'Nome do jogador' : kind === 'staff' ? 'Nome do treinador, olheiro...' : 'Nome do clube'
                  }
                  value={filter.query}
                  onChange={(e) => setFilter((f) => ({ ...f, query: e.target.value }))}
                />
                <div className="flex items-start gap-2 text-[14px] text-white">
                  <div className="flex flex-1 flex-col gap-0.5">
                    {(['players', 'staff', 'clubs'] as Kind[]).map((k) => (
                      <label key={k} className="flex items-center gap-2">
                        <input type="radio" checked={kind === k} onChange={() => (setKind(k), resetView())} />
                        {k === 'players' ? 'Jogadores' : k === 'staff' ? 'Staff' : 'Clubes'}
                      </label>
                    ))}
                  </div>
                  {kind === 'players' && (
                    <button type="button" className={`${TAB(filtersOpen)} h-7`} onClick={() => setFiltersOpen((o) => !o)}>
                      Filtros {filtersOpen ? '▴' : '▾'}
                    </button>
                  )}
                </div>
                {kind === 'players' && filtersOpen && <FilterBox filter={filter} options={options} onChange={setFilter} />}
                <button className="cm-purple h-8" disabled={busy}>
                  Procurar
                </button>
              </form>
              <button
                className="cm-purple h-8 enabled:text-cm-yellow"
                disabled={!dirty || busy}
                title={pending.length ? `Por salvar: ${pending.join(', ')}` : 'Nada por salvar'}
                onClick={() => void saveAll()}
              >
                Salvar Alterações{dirty ? ` (${pending.length})` : ''}
              </button>
              <button className="cm-purple h-8" disabled={!dirty || busy} onClick={() => setEdits(revertAll)}>
                Descartar Alterações
              </button>
              <button className="cm-purple h-8" disabled={kind !== 'players' || !hits.length || busy} onClick={() => setCommandOpen(true)}>
                Comando Personalizado
              </button>
              <button className="cm-purple h-8" disabled={kind !== 'players' || !hits.length || busy} onClick={() => void exportCsv()}>
                Exportar CSV
              </button>
              <button className="cm-purple h-8" onClick={() => guard(() => game.setMode('start'))}>
                Sair
              </button>
            </div>
            <div className="min-w-0 flex-1">
              {kind === 'staff' ? (
                selected !== null ? (
                  <StaffPanel file={file} id={selected} edit={edits.staff[selected]} onEdit={onStaffEdit} run={run} />
                ) : (
                  <div className="p-8 text-center text-cm-lilac">
                    Procure um treinador, olheiro ou fisioterapeuta pelo nome (ou deixe vazio para os melhores) e escolha na lista.
                  </div>
                )
              ) : kind === 'clubs' ? (
                <div className="p-8 text-center text-cm-lilac">
                  Procure um clube pelo nome e escolha na lista: abre o diálogo Editar Clube.
                </div>
              ) : !pe || !p || !view ? (
                <div className="p-8 text-center text-cm-lilac">
                  Procure um jogador pelo nome (ou deixe vazio para os melhores), use os filtros se quiser, e escolha na lista.
                </div>
              ) : (
                <>
                  <div className="mb-1.5 flex items-baseline gap-3">
                    <span className="font-title text-[22px] text-white">{pe.player.name}</span>
                    <span className="text-cm-magenta">{pe.player.club ?? 'Sem clube'}</span>
                    {playerDirty(pe) && <span className="text-[13px] text-cm-orange">Alterações por salvar</span>}
                    {notice && <span className="text-[13px] text-cm-yellow">{notice}</span>}
                    <span className="ml-auto font-mono text-[13px] text-cm-yellow">
                      CA {p.ability} · PA {p.potential}
                    </span>
                  </div>
                  <div className="mb-1.5 flex gap-1">
                    {(
                      [
                        ['data', 'Dados'],
                        ['attr1', 'Atributos 1'],
                        ['attr2', 'Atributos 2'],
                        ['contract', 'Contrato'],
                        ['prefs', 'Preferências'],
                      ] as [Tab, string][]
                    ).map(([t, label]) => (
                      <button key={t} className={TAB(tab === t)} onClick={() => setTab(t)}>
                        {label}
                      </button>
                    ))}
                    <button
                      className="ml-auto h-7 px-3 text-[13px] text-cm-lilac hover:text-white disabled:opacity-40"
                      disabled={!playerDirty(pe)}
                      title="Desfazer as mudanças deste jogador"
                      onClick={() => setPE((e) => ({ ...e, draft: {}, profile: e.view.profile }))}
                    >
                      Desfazer
                    </button>
                  </div>

                  {tab === 'data' && (
                    <div className="flex flex-wrap items-start gap-2">
                      <div className="flex w-[560px] flex-col gap-2">
                        <div className={BOX}>
                          <Header>Dados da pessoa</Header>
                          <DateField label="Data de nascimento" value={p.birthDate} onChange={(v) => setP({ birthDate: v })} />
                          <NationSelect label="Nação" value={p.nationId} nations={view.nations} onChange={(v) => setP({ nationId: v })} />
                          <NationSelect
                            label="2ª nação"
                            value={p.secondNationId}
                            nations={view.nations}
                            onChange={(v) => setP({ secondNationId: v })}
                          />
                          <div className="flex">
                            <div className="flex-1">
                              <NumField
                                label="Jogos pela seleção"
                                value={p.caps}
                                max={999}
                                width="w-16"
                                onChange={(v) => setP({ caps: v })}
                              />
                            </div>
                            <div className="flex-1">
                              <NumField label="Gols" value={p.goals} max={999} width="w-16" onChange={(v) => setP({ goals: v })} />
                            </div>
                          </div>
                        </div>
                        <div className={BOX}>
                          <Header>Posições (1–20)</Header>
                          <div className="flex gap-6">
                            <div className="flex-1">{POS_LEFT.map(positionField)}</div>
                            <div className="flex-1">{POS_RIGHT.map(positionField)}</div>
                          </div>
                        </div>
                        {pe.player.clubId != null && (
                          <div className="flex gap-1">
                            <button className="cm-purple h-8 flex-1" onClick={() => void showSquad(pe.player.clubId!)}>
                              Elenco: {pe.player.club}
                            </button>
                            <button
                              className="cm-purple h-8 flex-1"
                              onClick={() => (setClubIds([pe.player.clubId!]), setClubOpen(pe.player.clubId))}
                            >
                              Editar Clube: {pe.player.club}
                            </button>
                          </div>
                        )}
                      </div>
                      {p.morale != null && (
                        <div className={`${BOX} w-80`}>
                          <Header>Estado</Header>
                          <NumField
                            label="Moral (1–20)"
                            value={p.morale}
                            min={1}
                            max={20}
                            width="w-16"
                            onChange={(v) => setP({ morale: Math.min(20, Math.max(1, v)) })}
                          />
                          {p.condition != null && (
                            <NumField
                              label="Condição física (%)"
                              value={p.condition}
                              max={100}
                              width="w-16"
                              onChange={(v) => setP({ condition: Math.min(100, Math.max(0, v)) })}
                            />
                          )}
                          <label className={ROW}>
                            <span className="w-14 shrink-0">Lesão</span>
                            <input
                              className="cm-field h-6 min-w-0 flex-1 text-[13px]"
                              placeholder="Sem lesão"
                              value={p.injury ?? ''}
                              onChange={(e) => setP({ injury: e.target.value || null })}
                            />
                          </label>
                          {p.injury && (
                            <>
                              <DateField label="Recupera a" value={p.injuryUntil} onChange={(v) => setP({ injuryUntil: v })} />
                              <div className="flex justify-end px-2 py-1">
                                <button
                                  className="cm-purple h-7 px-3 text-[13px]"
                                  onClick={() => setP({ injury: null, injuryUntil: null })}
                                >
                                  Curar
                                </button>
                              </div>
                            </>
                          )}
                        </div>
                      )}
                    </div>
                  )}

                  {tab === 'attr1' && (
                    <>
                      <div className={BOX}>
                        <Header>Atributos de jogo</Header>
                        <div className="flex gap-4">
                          {columns.map((col, i) => (
                            <div key={i} className="min-w-0 flex-1">
                              {col.map(attrRow)}
                            </div>
                          ))}
                          <div className="flex w-36 shrink-0 flex-col gap-1">
                            <div className="flex items-center gap-2 text-[13px] text-white">
                              <span className="h-5 w-8 rounded bg-white/90" /> = Oculto
                            </div>
                            <button className="cm-purple h-7 text-[13px]" onClick={() => setAll(playingOf, 'equal')}>
                              Igualar Todos
                            </button>
                            <button className="cm-purple h-7 text-[13px]" onClick={() => setAll(playingOf, 'max')}>
                              Máx. Todos
                            </button>
                            <button className="cm-purple h-7 text-[13px]" onClick={() => setAll(playingOf, 'min')}>
                              Mín. Todos
                            </button>
                            <button
                              className="cm-purple h-7 text-[13px]"
                              title="Copiar todos os atributos deste jogador"
                              onClick={copyAttributes}
                            >
                              Copiar
                            </button>
                            <button
                              className="cm-purple h-7 text-[13px]"
                              disabled={!clip}
                              title={clip ? `Colar os atributos de ${clip.name}` : 'Copie primeiro os atributos de um jogador'}
                              onClick={pasteAttributes}
                            >
                              Colar
                            </button>
                          </div>
                        </div>
                      </div>
                      <div className={`${BOX} mt-2`}>
                        <Header>Reputações e habilidades</Header>
                        <div className="grid grid-cols-2 gap-x-6">
                          <NumField
                            label="Reputação local"
                            value={p.homeReputation}
                            max={10000}
                            onChange={(v) => setP({ homeReputation: v })}
                          />
                          <NumField label="Capacidade atual (CA)" value={p.ability} max={200} onChange={(v) => setP({ ability: v })} />
                          <NumField
                            label="Reputação atual"
                            value={p.currentReputation}
                            max={10000}
                            onChange={(v) => setP({ currentReputation: v })}
                          />
                          <NumField label="Potencial (PA)" value={p.potential} max={200} onChange={(v) => setP({ potential: v })} />
                          <NumField
                            label="Reputação mundial"
                            value={p.worldReputation}
                            max={10000}
                            onChange={(v) => setP({ worldReputation: v })}
                          />
                        </div>
                      </div>
                      <div className="pt-1.5 text-[12px] text-[#8a8a8a]">
                        Os valores são os que o jogo mostra (1 a 20); à direita, o valor salvo no save. Os destacados em branco o jogo
                        esconde.
                      </div>
                    </>
                  )}

                  {tab === 'attr2' && (
                    <div className={`${BOX} max-w-xl`}>
                      <Header>Atributos mentais</Header>
                      <div className="grid grid-cols-2 gap-x-6">{pe.player.attributes.filter(mentalOf).map(attrRow)}</div>
                      <div className="mt-2 flex justify-end gap-1">
                        <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setAll(mentalOf, 'max')}>
                          Máx. Todos
                        </button>
                        <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setAll(mentalOf, 'min')}>
                          Mín. Todos
                        </button>
                      </div>
                    </div>
                  )}

                  {tab === 'contract' && <ContractTab file={file} view={view} p={p} setP={setP} />}
                  {tab === 'prefs' && <PrefsTab file={file} list={p.favourites} onChange={(favourites) => setP({ favourites })} />}
                </>
              )}
            </div>
          </div>
        )}
      </DialogFrame>
      {busy && <Loading title="Editor" detail="Trabalhando no jogo salvo…" />}
      {clubOpen !== null && (
        <ClubDialog
          file={file}
          clubId={clubOpen}
          ids={clubIds}
          edit={edits.clubs[clubOpen]}
          onEdit={onClubEdit}
          onClose={() => setClubOpen(null)}
          onSquad={(id) => {
            setClubOpen(null)
            void showSquad(id)
          }}
          onSwitch={(id) => (setClubOpen(id), kind === 'clubs' && setSelected(id))}
        />
      )}
      {commandOpen && (
        <CommandDialog
          count={hits.length}
          person={pe?.player.name ?? null}
          onClose={() => setCommandOpen(false)}
          onRun={(commands, firstTeam, onlyPerson) => void runCommand(commands, firstTeam, onlyPerson)}
        />
      )}
      {leaving && (
        <Overlay>
          <DialogFrame title="Alterações por Salvar" width={560}>
            <div className="text-[15px] text-white">
              Há alterações não salvas em: <span className="text-cm-yellow">{pending.join(', ')}</span>.
            </div>
            <div className="mt-3 flex gap-1">
              <button className="cm-purple h-9 flex-1" onClick={() => setLeaving(null)}>
                Cancelar
              </button>
              <button
                className="cm-purple h-9 flex-1"
                onClick={() => {
                  const then = leaving
                  setLeaving(null)
                  setEdits(NO_EDITS)
                  then()
                }}
              >
                Continuar sem salvar
              </button>
              <button
                className="cm-purple h-9 flex-1 text-cm-yellow"
                onClick={() => {
                  const then = leaving
                  setLeaving(null)
                  void saveAll().then((ok) => ok && then())
                }}
              >
                Salvar e continuar
              </button>
            </div>
          </DialogFrame>
        </Overlay>
      )}
    </Overlay>
  )
}
