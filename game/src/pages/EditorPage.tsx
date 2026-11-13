import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import type { EditorAttribute } from '../bindings/EditorAttribute'
import type { EditorHit } from '../bindings/EditorHit'
import type { EditorPlayer } from '../bindings/EditorPlayer'
import type { EditorProfile } from '../bindings/EditorProfile'
import type { EditorProfileView } from '../bindings/EditorProfileView'
import type { SaveSummary } from '../bindings/SaveSummary'
import { DialogFrame, Overlay } from '../components/Dialog'
import Loading from '../components/Loading'
import { ErrorBox } from '../components/Screen'
import { useGame } from '../game'
import ClubDialog from './editor/ClubDialog'
import StaffPanel from './editor/StaffPanel'
import CommandDialog from './editor/CommandDialog'
import { BOX, LABELS, POS_LEFT, POS_RIGHT, POSITION_LABELS, TAB } from './editor/editorText'
import { AttributeRow, DateField, Header, NationSelect, NumField } from './editor/Fields'

// Editor de jogos salvos (marco 3.20, primeira fatia): procura um jogador num save fechado e
// edita os atributos pelo valor de 1 a 20 que o jogo mostra. O core converte para o valor
// guardado (ao lado, só de leitura); os atributos ocultos vão a branco, como no editor do CM.

type Kind = 'players' | 'staff' | 'clubs'
type Tab = 'data' | 'attr1' | 'attr2' | 'contract'
type Sub = 'details' | 'unhappy' | 'club'

/** "Editor de Jogos Salvos": no formato do editor do CM (lista à esquerda, abas à direita). */
export default function EditorPage() {
  const game = useGame()
  const [saves, setSaves] = useState<SaveSummary[]>([])
  const [file, setFile] = useState('')
  const [kind, setKind] = useState<Kind>('players')
  const [tab, setTab] = useState<Tab>('data')
  const [sub, setSub] = useState<Sub>('details')
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<EditorHit[]>([])
  const [player, setPlayer] = useState<EditorPlayer | null>(null)
  const [draft, setDraft] = useState<Record<string, number>>({})
  const [profile, setProfile] = useState<EditorProfileView | null>(null)
  const [profileDraft, setProfileDraft] = useState<EditorProfile | null>(null)
  const [clubOpen, setClubOpen] = useState<number | null>(null)
  const [commandOpen, setCommandOpen] = useState(false)
  // Copiar/Colar atributos: os valores mostrados de um jogador, para colar no rascunho de outro
  const [clip, setClip] = useState<{ name: string; values: Record<string, number> } | null>(null)
  const [selected, setSelected] = useState<number | null>(null)
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

  const run = async <T,>(job: () => Promise<T>): Promise<T | undefined> => {
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
  }

  const reset = () => {
    setHits([])
    setPlayer(null)
    setProfile(null)
    setProfileDraft(null)
    setClubOpen(null)
    setSelected(null)
    setDraft({})
    setNotice(null)
  }

  const search = async () => {
    if (!file) return
    const found = await run(() =>
      kind === 'players'
        ? api.editorSearch(file, query)
        : kind === 'staff'
          ? api.editorSearchStaff(file, query)
          : api.editorSearchClubs(file, query),
    )
    if (found) {
      setHits(found)
      setNotice(found.length ? null : kind === 'clubs' ? 'Nenhum clube encontrado.' : 'Ninguém encontrado.')
    }
  }

  /** Lista todos os jogadores do clube e passa para o modo Jogadores. */
  const showSquad = async (clubId: number) => {
    const found = await run(() => api.editorSquad(file, clubId))
    if (found) {
      reset()
      setKind('players')
      setHits(found)
      setNotice(found.length ? null : 'O clube não tem jogadores.')
    }
  }

  const open = async (id: number) => {
    if (kind === 'clubs') {
      setSelected(id)
      setClubOpen(id)
      return
    }
    if (kind === 'staff') {
      setSelected(id)
      setNotice(null)
      return
    }
    const both = await run(() => Promise.all([api.editorPlayer(file, id), api.editorProfile(file, id)]))
    if (both) {
      setSelected(id)
      setNotice(null)
      setPlayer(both[0])
      setDraft({})
      setProfile(both[1])
      setProfileDraft(both[1].profile)
    }
  }

  const changes = useMemo(
    () => (player ? player.attributes.filter((a) => draft[a.key] !== undefined && draft[a.key] !== a.shown) : []),
    [player, draft],
  )
  const profileChanged = !!profile && !!profileDraft && JSON.stringify(profile.profile) !== JSON.stringify(profileDraft)
  const dirty = changes.length > 0 || profileChanged

  const update = async () => {
    if (!player) return
    // A ficha primeiro: se o core a recusar, os atributos ficam por gravar e o erro aparece
    if (profileChanged && profileDraft) {
      const v = await run(() => api.editorSetProfile(file, player.id, profileDraft))
      if (!v) return
      setProfile(v)
      setProfileDraft(v.profile)
    }
    if (changes.length) {
      const p = await run(() =>
        api.editorSetAttributes(
          file,
          player.id,
          changes.map((a) => ({ key: a.key, value: draft[a.key] ?? a.shown })),
        ),
      )
      if (!p) return
      setPlayer(p)
      setDraft({})
    }
    setNotice(`${player.name}: gravado.`)
  }

  /** Max/Min/Igualar: aplica a todos os atributos do conjunto (Igualar = a média arredondada). */
  const setAll = (group: (a: EditorAttribute) => boolean, how: 'max' | 'min' | 'equal') => {
    if (!player) return
    const list = player.attributes.filter(group)
    const mean = Math.round(list.reduce((s, a) => s + (draft[a.key] ?? a.shown), 0) / Math.max(1, list.length))
    const value = how === 'max' ? 20 : how === 'min' ? 1 : mean
    setDraft((d) => {
      const next = { ...d }
      for (const a of list) next[a.key] = value
      return next
    })
  }

  /** Copia os atributos do jogador aberto (com o que estiver por gravar). */
  const copyAttributes = () => {
    if (!player) return
    setClip({ name: player.name, values: Object.fromEntries(player.attributes.map((a) => [a.key, draft[a.key] ?? a.shown])) })
    setNotice(`Atributos de ${player.name} copiados.`)
  }

  /** Cola no rascunho do jogador aberto: só grava com o Actualizar Jogador (e o core valida). */
  const pasteAttributes = () => {
    if (!player || !clip) return
    const next: Record<string, number> = {}
    for (const a of player.attributes) {
      const v = clip.values[a.key]
      if (v !== undefined) next[a.key] = v
    }
    setDraft(next)
    setNotice(`Atributos de ${clip.name} colados: Actualizar Jogador grava-os.`)
  }

  /** Exporta a lista da esquerda para CSV, ao lado dos saves. */
  const exportCsv = async () => {
    const path = await run(() =>
      api.editorExportCsv(
        file,
        hits.map((h) => h.id),
      ),
    )
    if (path) setNotice(`Exportado para ${path}`)
  }

  /** Corre o Comando Personalizado e reabre o jogador aberto, para se ver o que mudou. */
  const runCommand = async (commands: string[], firstTeam: boolean, onlyPerson: boolean) => {
    setCommandOpen(false)
    const ids = onlyPerson && player ? [player.id] : hits.map((h) => h.id)
    const n = await run(() => api.editorCommand(file, ids, commands, firstTeam))
    if (n === undefined) return
    if (player) await open(player.id)
    setNotice(`Comando aplicado a ${n} jogador${n === 1 ? '' : 'es'}.`)
  }

  const pd = profileDraft
  const setP = (patch: Partial<EditorProfile>) => setProfileDraft((d) => (d ? { ...d, ...patch } : d))
  const setUnhappy = (club: boolean, mode: 'all' | 'none') => {
    if (!profile || !pd) return
    const keys = profile.reasons.filter((r) => r.club === club).map((r) => r.key)
    const rest = pd.unhappy.filter((k) => !keys.includes(k))
    setP({ unhappy: mode === 'all' ? [...rest, ...keys] : rest })
  }
  const toggleReason = (key: string, on: boolean) => {
    if (!pd) return
    setP({ unhappy: on ? [...pd.unhappy.filter((k) => k !== key), key] : pd.unhappy.filter((k) => k !== key) })
  }

  const mentalOf = (a: EditorAttribute) => a.group === 'mental'
  const playingOf = (a: EditorAttribute) => a.group !== 'mental'
  // Os atributos de jogo, por ordem alfabética do rótulo, repartidos em três colunas como no print
  const playing = useMemo(
    () =>
      player
        ? player.attributes.filter(playingOf).sort((a, b) => (LABELS[a.key] ?? a.key).localeCompare(LABELS[b.key] ?? b.key, 'pt'))
        : [],
    [player],
  )
  const per = Math.ceil(playing.length / 3)
  const columns = [playing.slice(0, per), playing.slice(per, per * 2), playing.slice(per * 2)]
  const attrRow = (a: EditorAttribute) => (
    <AttributeRow key={a.key} a={a} value={draft[a.key] ?? a.shown} onChange={(v) => setDraft((d) => ({ ...d, [a.key]: v }))} />
  )

  const reasonsBox = (club: boolean) =>
    profile && pd ? (
      <div className={BOX}>
        <Header>{club ? 'Estado de contentamento com o clube' : 'Estado de insatisfação'}</Header>
        <div className="grid grid-cols-2 gap-x-6">
          {profile.reasons
            .filter((r) => r.club === club)
            .map((r) => (
              <label key={r.key} className="flex h-7 items-center gap-2 text-[14px] text-cm-yellow">
                <input type="checkbox" checked={pd.unhappy.includes(r.key)} onChange={(e) => toggleReason(r.key, e.target.checked)} />
                {r.label}
              </label>
            ))}
        </div>
        <div className="mt-2 flex justify-end gap-1">
          <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setUnhappy(club, 'all')}>
            Marcar todos
          </button>
          <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => setUnhappy(club, 'none')}>
            Limpar todos
          </button>
        </div>
      </div>
    ) : null

  return (
    <Overlay>
      <DialogFrame title="Editor de Jogos Salvos" full>
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {saves.length === 0 ? (
          <div className="p-6 text-center text-cm-yellow">Nenhum jogo gravado para editar.</div>
        ) : (
          <div className="flex h-full gap-3">
            <div className="flex w-80 shrink-0 flex-col gap-1.5">
              <select className="cm-field h-7 text-[14px]" value={file} onChange={(e) => (setFile(e.target.value), reset())}>
                {saves.map((s) => (
                  <option key={s.file} value={s.file}>
                    {s.name} · {s.clubName}
                  </option>
                ))}
              </select>
              <div className="min-h-[200px] flex-1 overflow-auto rounded border border-white/15 bg-black/30">
                {hits.map((h) => (
                  <button
                    key={h.id}
                    onClick={() => void open(h.id)}
                    className={`flex h-6 w-full items-center gap-2 px-2 text-left text-[13.5px] hover:bg-cm-sel ${selected === h.id ? 'bg-cm-sel text-white' : 'text-cm-yellow'}`}
                  >
                    <span className="min-w-0 flex-1 truncate">{h.name}</span>
                    <span className="w-9 text-right font-mono text-[12px]">{h.ability ?? ''}</span>
                  </button>
                ))}
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
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
                <div className="flex flex-col gap-0.5 text-[14px] text-white">
                  {(['players', 'staff', 'clubs'] as Kind[]).map((k) => (
                    <label key={k} className="flex items-center gap-2">
                      <input type="radio" checked={kind === k} onChange={() => (setKind(k), reset())} />
                      {k === 'players' ? 'Jogadores' : k === 'staff' ? 'Staff' : 'Clubes'}
                    </label>
                  ))}
                </div>
                <button className="cm-purple h-8" disabled={busy}>
                  Procurar
                </button>
              </form>
              <button className="cm-purple h-8 enabled:text-cm-yellow" disabled={!dirty || busy} onClick={() => void update()}>
                Actualizar Jogador
              </button>
              <button className="cm-purple h-8" disabled={kind !== 'players' || !hits.length || busy} onClick={() => setCommandOpen(true)}>
                Comando Personalizado
              </button>
              <button className="cm-purple h-8" disabled={kind !== 'players' || !hits.length || busy} onClick={() => void exportCsv()}>
                Exportar CSV
              </button>
              <button className="cm-purple h-8" onClick={() => game.setMode('start')}>
                Sair
              </button>
            </div>
            <div className="min-w-0 flex-1">
              {kind === 'staff' ? (
                selected !== null ? (
                  <StaffPanel file={file} id={selected} run={run} />
                ) : (
                  <div className="p-8 text-center text-cm-lilac">
                    Procure um treinador, olheiro ou fisioterapeuta pelo nome (ou deixe vazio para os melhores) e escolha-o na lista.
                  </div>
                )
              ) : kind === 'clubs' ? (
                <div className="p-8 text-center text-cm-lilac">
                  Procure um clube pelo nome e escolha-o na lista: abre o diálogo Editar Clube.
                </div>
              ) : !player || !pd || !profile ? (
                <div className="p-8 text-center text-cm-lilac">
                  Procure um jogador pelo nome (ou deixe vazio para os melhores) e escolha-o na lista.
                </div>
              ) : (
                <>
                  <div className="mb-1.5 flex items-baseline gap-3">
                    <span className="font-title text-[22px] text-white">{player.name}</span>
                    <span className="text-cm-magenta">{player.club ?? 'Sem clube'}</span>
                    {notice && <span className="text-[13px] text-cm-yellow">{notice}</span>}
                    <span className="ml-auto font-mono text-[13px] text-cm-yellow">
                      CA {pd.ability} · PA {pd.potential}
                    </span>
                  </div>
                  <div className="mb-1.5 flex gap-1">
                    {(
                      [
                        ['data', 'Dados'],
                        ['attr1', 'Atributos 1'],
                        ['attr2', 'Atributos 2'],
                        ['contract', 'Contrato'],
                      ] as [Tab, string][]
                    ).map(([t, label]) => (
                      <button key={t} className={TAB(tab === t)} onClick={() => setTab(t)}>
                        {label}
                      </button>
                    ))}
                  </div>

                  {tab === 'data' && (
                    <div className="flex flex-col gap-2">
                      <div className={`${BOX} max-w-xl`}>
                        <Header>Dados da pessoa</Header>
                        <DateField label="Data de nascimento" value={pd.birthDate} onChange={(v) => setP({ birthDate: v })} />
                        <NationSelect label="Nação" value={pd.nationId} nations={profile.nations} onChange={(v) => setP({ nationId: v })} />
                        <NationSelect
                          label="2ª nação"
                          value={pd.secondNationId}
                          nations={profile.nations}
                          onChange={(v) => setP({ secondNationId: v })}
                        />
                        <div className="flex">
                          <div className="flex-1">
                            <NumField
                              label="Internacionalizações"
                              value={pd.caps}
                              max={999}
                              width="w-16"
                              onChange={(v) => setP({ caps: v })}
                            />
                          </div>
                          <div className="flex-1">
                            <NumField label="Golos" value={pd.goals} max={999} width="w-16" onChange={(v) => setP({ goals: v })} />
                          </div>
                        </div>
                      </div>
                      <div className={`${BOX} max-w-xl`}>
                        <Header>Posições (1–20)</Header>
                        <div className="flex gap-6">
                          <div className="flex-1">
                            {POS_LEFT.map((k) => (
                              <NumField
                                key={k}
                                label={POSITION_LABELS[k] ?? k}
                                value={pd.positions[profile.positionKeys.indexOf(k)] ?? 1}
                                min={1}
                                max={20}
                                width="w-16"
                                onChange={(v) =>
                                  setP({
                                    positions: pd.positions.map((x, j) =>
                                      j === profile.positionKeys.indexOf(k) ? Math.min(20, Math.max(1, v)) : x,
                                    ),
                                  })
                                }
                              />
                            ))}
                          </div>
                          <div className="flex-1">
                            {POS_RIGHT.map((k) => (
                              <NumField
                                key={k}
                                label={POSITION_LABELS[k] ?? k}
                                value={pd.positions[profile.positionKeys.indexOf(k)] ?? 1}
                                min={1}
                                max={20}
                                width="w-16"
                                onChange={(v) =>
                                  setP({
                                    positions: pd.positions.map((x, j) =>
                                      j === profile.positionKeys.indexOf(k) ? Math.min(20, Math.max(1, v)) : x,
                                    ),
                                  })
                                }
                              />
                            ))}
                          </div>
                        </div>
                      </div>
                      {player.clubId != null && (
                        <div className="flex max-w-xl gap-1">
                          <button className="cm-purple h-8 flex-1" onClick={() => void showSquad(player.clubId!)}>
                            Plantel e Staff: {player.club}
                          </button>
                          <button className="cm-purple h-8 flex-1" onClick={() => setClubOpen(player.clubId)}>
                            Editar Clube: {player.club}
                          </button>
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
                            {pd.morale != null && (
                              <NumField
                                label="Moral"
                                value={pd.morale}
                                min={1}
                                max={20}
                                width="w-14"
                                onChange={(v) => setP({ morale: Math.min(20, Math.max(1, v)) })}
                              />
                            )}
                          </div>
                        </div>
                      </div>
                      <div className={`${BOX} mt-2`}>
                        <Header>Reputações e habilidades</Header>
                        <div className="grid grid-cols-2 gap-x-6">
                          <NumField
                            label="Reputação local"
                            value={pd.homeReputation}
                            max={10000}
                            onChange={(v) => setP({ homeReputation: v })}
                          />
                          <NumField label="Capacidade actual (CA)" value={pd.ability} max={200} onChange={(v) => setP({ ability: v })} />
                          <NumField
                            label="Reputação actual"
                            value={pd.currentReputation}
                            max={10000}
                            onChange={(v) => setP({ currentReputation: v })}
                          />
                          <NumField label="Potencial (PA)" value={pd.potential} max={200} onChange={(v) => setP({ potential: v })} />
                          <NumField
                            label="Reputação mundial"
                            value={pd.worldReputation}
                            max={10000}
                            onChange={(v) => setP({ worldReputation: v })}
                          />
                        </div>
                      </div>
                      <div className="pt-1.5 text-[12px] text-[#8a8a8a]">
                        Os valores são os que o jogo mostra (1 a 20); à direita, o valor guardado no save. Os destacados a branco o jogo
                        esconde.
                      </div>
                    </>
                  )}

                  {tab === 'attr2' && (
                    <div className={`${BOX} max-w-xl`}>
                      <Header>Atributos mentais</Header>
                      <div className="grid grid-cols-2 gap-x-6">{player.attributes.filter(mentalOf).map(attrRow)}</div>
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

                  {tab === 'contract' && (
                    <>
                      <div className="mb-1.5 flex gap-1">
                        {(
                          [
                            ['details', 'Detalhes do Contrato'],
                            ['unhappy', 'Insatisfação'],
                            ['club', 'Insatisfação com o Clube'],
                          ] as [Sub, string][]
                        ).map(([s, label]) => (
                          <button key={s} className={TAB(sub === s)} onClick={() => setSub(s)}>
                            {label}
                          </button>
                        ))}
                      </div>
                      {sub === 'details' && (
                        <div className={`${BOX} max-w-xl`}>
                          <Header>Detalhes do contrato</Header>
                          <NumField
                            label="Salário semanal"
                            value={pd.wage}
                            max={100000000}
                            width="w-36"
                            onChange={(v) => setP({ wage: v })}
                          />
                          <DateField label="Início" value={pd.contractStart} onChange={(v) => setP({ contractStart: v })} />
                          <DateField label="Termina" value={pd.contractExpires} onChange={(v) => setP({ contractExpires: v })} />
                        </div>
                      )}
                      {sub === 'unhappy' && reasonsBox(false)}
                      {sub === 'club' && reasonsBox(true)}
                    </>
                  )}
                </>
              )}
            </div>
          </div>
        )}
      </DialogFrame>
      {busy && <Loading title="Editor" detail="A trabalhar no jogo gravado…" />}
      {clubOpen !== null && (
        <ClubDialog
          file={file}
          clubId={clubOpen}
          ids={kind === 'clubs' ? hits.map((h) => h.id) : []}
          onClose={() => setClubOpen(null)}
          onSquad={(id) => {
            setClubOpen(null)
            void showSquad(id)
          }}
          onSwitch={(id) => (setClubOpen(id), setSelected(id))}
        />
      )}
      {commandOpen && (
        <CommandDialog
          count={hits.length}
          person={player?.name ?? null}
          onClose={() => setCommandOpen(false)}
          onRun={(commands, firstTeam, onlyPerson) => void runCommand(commands, firstTeam, onlyPerson)}
        />
      )}
    </Overlay>
  )
}
