import { useEffect, useState } from 'react'
import { api } from '../../api'
import type { EditorClub } from '../../bindings/EditorClub'
import { DialogFrame, Overlay } from '../../components/Dialog'
import Loading from '../../components/Loading'
import { ErrorBox } from '../../components/Screen'
import { BOX, ROW, STATUS } from './editorText'
import { ColorField, Header, NumField } from './Fields'

/** Diálogo "Editar Clube" (como o Edit Club do editor do CM) com |< << >> >| entre os clubes da lista. */
export default function ClubDialog({
  file,
  clubId,
  ids,
  onClose,
  onSquad,
  onSwitch,
}: {
  file: string
  clubId: number
  ids: number[]
  onClose: () => void
  onSquad: (id: number) => void
  onSwitch: (id: number) => void
}) {
  const [saved, setSaved] = useState<EditorClub | null>(null)
  const [draft, setDraft] = useState<EditorClub | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    let live = true
    api.editorClub(file, clubId).then(
      (c) => {
        if (!live) return
        setSaved(c)
        setDraft(c)
        setError(null)
        setNotice(null)
      },
      (e) => live && setError(String(e)),
    )
    return () => {
      live = false
    }
  }, [file, clubId])

  const dirty = !!saved && !!draft && JSON.stringify(saved) !== JSON.stringify(draft)
  const set = (patch: Partial<EditorClub>) => setDraft((d) => (d ? { ...d, ...patch } : d))

  const update = async (): Promise<boolean> => {
    if (!draft) return false
    setBusy(true)
    setError(null)
    try {
      const c = await api.editorSetClub(file, draft)
      setSaved(c)
      setDraft(c)
      setNotice('Gravado.')
      return true
    } catch (e) {
      setError(String(e))
      return false
    } finally {
      setBusy(false)
    }
  }
  const go = async (id: number | undefined) => {
    if (id === undefined || id === clubId) return
    if (dirty && !(await update())) return
    onSwitch(id)
  }
  const at = ids.indexOf(clubId)
  const nav = at >= 0 && ids.length > 1

  return (
    <Overlay>
      <DialogFrame title="Editar Clube" full={false} width={760} onCancel={undefined}>
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {!draft ? (
          <Loading title="Clube" detail="A abrir…" />
        ) : (
          <div className="flex flex-col gap-2 text-white">
            <div className="flex items-baseline gap-3">
              <span className="font-title text-[22px]">{draft.name}</span>
              {notice && <span className="text-[13px] text-cm-yellow">{notice}</span>}
              <button className="cm-purple ml-auto h-7 px-3 text-[13px]" onClick={() => onSquad(draft.id)}>
                Listar jogadores na janela principal
              </button>
            </div>
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
                <NumField label="Saldo bancário" value={draft.cash} onChange={(v) => set({ cash: v })} width="w-36" />
              </div>
              <div className={`${BOX} min-w-0 flex-1`}>
                <Header>Estádio · {draft.stadiumName ?? 'sem estádio'}</Header>
                <NumField label="Capacidade" value={draft.capacity} max={200000} onChange={(v) => set({ capacity: v })} />
                <NumField label="Lugares sentados" value={draft.seating} max={200000} onChange={(v) => set({ seating: v })} />
                <NumField label="Expansão" value={draft.expansion} max={200000} onChange={(v) => set({ expansion: v })} />
                <label className={ROW}>
                  <span className="flex-1">Aquecimento subterrâneo</span>
                  <input type="checkbox" checked={draft.undersoilHeating} onChange={(e) => set({ undersoilHeating: e.target.checked })} />
                </label>
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
            <div className="flex gap-1">
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at === 0} onClick={() => void go(ids[0])}>
                |&lt;
              </button>
              <button className="cm-purple h-8 w-16 disabled:opacity-40" disabled={!nav || at <= 0} onClick={() => void go(ids[at - 1])}>
                &lt;&lt;
              </button>
              <button
                className="cm-purple h-8 w-16 disabled:opacity-40"
                disabled={!nav || at >= ids.length - 1}
                onClick={() => void go(ids[at + 1])}
              >
                &gt;&gt;
              </button>
              <button
                className="cm-purple h-8 w-16 disabled:opacity-40"
                disabled={!nav || at >= ids.length - 1}
                onClick={() => void go(ids[ids.length - 1])}
              >
                &gt;|
              </button>
              <span className="flex-1" />
              <button
                className="cm-purple h-8 w-28 disabled:opacity-40 enabled:text-cm-yellow"
                disabled={!dirty || busy}
                onClick={() => void update()}
              >
                Actualizar
              </button>
              <button
                className="cm-purple h-8 w-28"
                onClick={() => {
                  void (async () => {
                    if (!dirty || (await update())) onClose()
                  })()
                }}
              >
                OK
              </button>
            </div>
          </div>
        )}
      </DialogFrame>
    </Overlay>
  )
}
