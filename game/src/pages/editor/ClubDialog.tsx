import { useEffect, useState } from 'react'
import { api } from '../../api'
import type { EditorClub } from '../../bindings/EditorClub'
import { DialogFrame, Overlay } from '../../components/Dialog'
import Loading from '../../components/Loading'
import { ErrorBox } from '../../components/Screen'
import { clubDirty, type ClubEdit } from './editorEdits'
import { BOX, ROW, STATUS, TAB } from './editorText'
import { ColorField, Header, NumField, RefList } from './Fields'
import PickDialog from './PickDialog'

type Pick = 'liked' | 'disliked' | 'rival'

/** Diálogo "Editar Clube" (como o Edit Club do editor do CM) com |< << >> >| entre os clubes da
 * lista. As mudanças ficam por gravar no editor (o "Gravar Alterações" grava tudo de uma vez). */
export default function ClubDialog({
  file,
  clubId,
  ids,
  edit,
  onEdit,
  onClose,
  onSquad,
  onSwitch,
}: {
  file: string
  clubId: number
  ids: number[]
  /** O que já se abriu (e mudou) deste clube nesta sessão do editor. */
  edit: ClubEdit | undefined
  onEdit: (e: ClubEdit) => void
  onClose: () => void
  onSquad: (id: number) => void
  onSwitch: (id: number) => void
}) {
  const [error, setError] = useState<string | null>(null)
  const [tab, setTab] = useState<'club' | 'wages'>('club')
  const [pick, setPick] = useState<Pick | null>(null)

  useEffect(() => {
    if (edit) return
    let live = true
    api.editorClub(file, clubId).then(
      (c) => live && onEdit({ saved: c, draft: c }),
      (e) => live && setError(String(e)),
    )
    return () => {
      live = false
    }
  }, [file, clubId, edit, onEdit])

  const draft = edit?.draft
  const set = (patch: Partial<EditorClub>) => edit && onEdit({ ...edit, draft: { ...edit.draft, ...patch } })
  const at = ids.indexOf(clubId)
  const nav = at >= 0 && ids.length > 1
  const go = (id: number | undefined) => id !== undefined && id !== clubId && onSwitch(id)
  const people = (liked: boolean) =>
    (draft?.favourites ?? []).filter((f) => f.liked === liked).map((f) => ({ id: f.targetId, name: f.name }))
  const dropPerson = (liked: boolean, id?: number) =>
    draft && set({ favourites: draft.favourites.filter((f) => f.liked !== liked || (id !== undefined && f.targetId !== id)) })
  const bill = (draft?.wages ?? []).reduce((s, w) => s + w.wage, 0)
  const scaleWages = (pct: number) =>
    draft && set({ wages: draft.wages.map((w) => ({ ...w, wage: Math.max(0, Math.round((w.wage * (100 + pct)) / 100)) })) })

  return (
    <Overlay>
      <DialogFrame title="Editar Clube" full={false} width={860} onCancel={undefined}>
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {!draft || !edit ? (
          <Loading title="Clube" detail="A abrir…" />
        ) : (
          <div className="flex flex-col gap-2 text-white">
            <div className="flex items-baseline gap-3">
              <span className="font-title text-[22px]">{draft.name}</span>
              {clubDirty(edit) && <span className="text-[13px] text-cm-yellow">Alterações por gravar</span>}
              <button className="cm-purple ml-auto h-7 px-3 text-[13px]" onClick={() => onSquad(draft.id)}>
                Listar jogadores na janela principal
              </button>
            </div>
            <div className="flex gap-1">
              <button className={TAB(tab === 'club')} onClick={() => setTab('club')}>
                Clube
              </button>
              <button className={TAB(tab === 'wages')} onClick={() => setTab('wages')}>
                Folha Salarial
              </button>
            </div>

            {tab === 'club' && (
              <>
                <div className="flex gap-2">
                  <div className={`${BOX} min-w-0 flex-1`}>
                    <Header>Dados gerais</Header>
                    <label className={ROW}>
                      <span className="flex-1">Estatuto profissional</span>
                      <select
                        className="cm-field h-6 w-40 text-[14px]"
                        value={draft.professionalStatus}
                        onChange={(e) => set({ professionalStatus: Number(e.target.value) })}
                      >
                        {STATUS.map(([v, n]) => (
                          <option key={v} value={v}>
                            {n}
                          </option>
                        ))}
                      </select>
                    </label>
                    <NumField label="Treino (1–20)" value={draft.training} min={1} max={20} onChange={(v) => set({ training: v })} />
                    <NumField label="Reputação" value={draft.reputation} max={10000} onChange={(v) => set({ reputation: v })} />
                    <NumField label="Saldo bancário" value={draft.cash} onChange={(v) => set({ cash: v })} width="w-32" />
                    {draft.wageBudget != null && (
                      <NumField
                        label="Folha permitida (semanal)"
                        value={draft.wageBudget}
                        onChange={(v) => set({ wageBudget: v })}
                        width="w-32"
                      />
                    )}
                    {draft.maxWage != null && (
                      <div className={ROW}>
                        <span className="flex-1">Salário máximo actual</span>
                        <span className="w-32 pr-2 text-right font-mono text-cm-yellow">{draft.maxWage}</span>
                      </div>
                    )}
                  </div>
                  <div className={`${BOX} min-w-0 flex-1`}>
                    <Header>Estádio · {draft.stadiumName ?? 'sem estádio'}</Header>
                    <NumField label="Capacidade" value={draft.capacity} max={200000} onChange={(v) => set({ capacity: v })} />
                    <NumField label="Lugares sentados" value={draft.seating} max={200000} onChange={(v) => set({ seating: v })} />
                    <NumField label="Expansão" value={draft.expansion} max={200000} onChange={(v) => set({ expansion: v })} />
                    <label className={ROW}>
                      <span className="flex-1">Aquecimento subterrâneo</span>
                      <input
                        type="checkbox"
                        checked={draft.undersoilHeating}
                        onChange={(e) => set({ undersoilHeating: e.target.checked })}
                      />
                    </label>
                  </div>
                </div>
                <div className={BOX}>
                  <Header>Preferências de jogadores/staff e rivais</Header>
                  <div className="flex gap-3">
                    <RefList
                      title="Jogadores/staff favoritos"
                      items={people(true)}
                      onAdd={() => setPick('liked')}
                      onRemove={(id) => dropPerson(true, id)}
                      onClear={() => dropPerson(true)}
                    />
                    <RefList
                      title="Jogadores/staff detestados"
                      items={people(false)}
                      onAdd={() => setPick('disliked')}
                      onRemove={(id) => dropPerson(false, id)}
                      onClear={() => dropPerson(false)}
                    />
                    <RefList
                      title="Clubes rivais"
                      items={draft.rivals}
                      onAdd={() => setPick('rival')}
                      onRemove={(id) => set({ rivals: draft.rivals.filter((r) => r.id !== id) })}
                      onClear={() => set({ rivals: [] })}
                    />
                  </div>
                </div>
                <div className={BOX}>
                  <Header>Equipamentos</Header>
                  <div className="flex gap-3">
                    <div className="flex-1">
                      <ColorField label="Principal · texto" value={draft.homeFg} onChange={(v) => set({ homeFg: v })} />
                      <ColorField label="Principal · fundo" value={draft.homeBg} onChange={(v) => set({ homeBg: v })} />
                    </div>
                    <div className="flex-1">
                      <ColorField label="Alternativo · texto" value={draft.awayFg} onChange={(v) => set({ awayFg: v })} />
                      <ColorField label="Alternativo · fundo" value={draft.awayBg} onChange={(v) => set({ awayBg: v })} />
                    </div>
                  </div>
                </div>
              </>
            )}

            {tab === 'wages' && (
              <div className={BOX}>
                <Header>Folha salarial (por semana)</Header>
                <div className="cm-zebra h-[400px] overflow-auto">
                  {draft.wages.map((w, i) => (
                    <label key={w.id} className="flex h-7 items-center gap-2 px-2 text-[14px]">
                      <span className="min-w-0 flex-1 truncate">{w.name}</span>
                      <span className="w-16 text-[12px] text-cm-lilac">{w.player ? 'Jogador' : 'Staff'}</span>
                      <input
                        type="number"
                        min={0}
                        value={w.wage}
                        onChange={(e) =>
                          set({ wages: draft.wages.map((x, j) => (j === i ? { ...x, wage: Number(e.target.value) || 0 } : x)) })
                        }
                        className={`cm-field h-6 w-32 text-right text-[14px] ${w.wage !== edit.saved.wages[i]?.wage ? '!text-cm-yellow' : ''}`}
                      />
                    </label>
                  ))}
                </div>
                <div className="mt-2 flex items-center gap-1">
                  <span className="text-[14px]">
                    Total: <span className="font-mono text-cm-yellow">{bill}</span>
                  </span>
                  <span className="flex-1" />
                  <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => scaleWages(-10)}>
                    Todos −10%
                  </button>
                  <button className="cm-purple h-7 w-28 text-[13px]" onClick={() => scaleWages(10)}>
                    Todos +10%
                  </button>
                </div>
              </div>
            )}

            <div className="flex gap-1">
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at === 0} onClick={() => go(ids[0])}>
                |&lt;
              </button>
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at <= 0} onClick={() => go(ids[at - 1])}>
                &lt;&lt;
              </button>
              <button
                className="cm-purple h-8 w-16 disabled:opacity-40"
                disabled={!nav || at >= ids.length - 1}
                onClick={() => go(ids[at + 1])}
              >
                &gt;&gt;
              </button>
              <button
                className="cm-purple h-8 w-16 disabled:opacity-40"
                disabled={!nav || at >= ids.length - 1}
                onClick={() => go(ids[ids.length - 1])}
              >
                &gt;|
              </button>
              <span className="flex-1" />
              <button
                className="cm-purple h-8 w-28 disabled:opacity-40"
                disabled={!clubDirty(edit)}
                title="Desfazer as mudanças deste clube"
                onClick={() => onEdit({ ...edit, draft: edit.saved })}
              >
                Desfazer
              </button>
              <button className="cm-purple h-8 w-28 text-cm-yellow" onClick={onClose}>
                OK
              </button>
            </div>
          </div>
        )}
      </DialogFrame>
      {pick && edit && draft && (
        <PickDialog
          file={file}
          kind={pick === 'rival' ? 'club' : 'person'}
          title={pick === 'rival' ? 'Clube Rival' : pick === 'liked' ? 'Favorito do Clube' : 'Detestado pelo Clube'}
          onClose={() => setPick(null)}
          onPick={(h) => {
            setPick(null)
            if (pick === 'rival') {
              if (!draft.rivals.some((r) => r.id === h.id)) set({ rivals: [...draft.rivals, { id: h.id, name: h.name }] })
            } else {
              const rest = draft.favourites.filter((f) => f.targetId !== h.id)
              set({ favourites: [...rest, { kind: 'person', targetId: h.id, name: h.name, liked: pick === 'liked' }] })
            }
          }}
        />
      )}
    </Overlay>
  )
}
