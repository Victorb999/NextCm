import { useEffect, useState } from 'react'
import { api } from '../../api'
import type { EditorHit } from '../../bindings/EditorHit'
import type { EditorStaffData } from '../../bindings/EditorStaffData'
import { staffDirty, type StaffEdit } from './editorEdits'
import { BOX, LABELS, ROW } from './editorText'
import { AttributeRow, Header, NumField } from './Fields'

// O staff no Editor de Jogos Salvos (marco 4.0, item 16): a ficha (CA/PA, reputações, clube,
// função, salário e contrato) e os atributos de staff e mentais, pelo valor que o jogo mostra.
// A validação e a conversão dos relativos ficam no core (`editor_staff.rs`). As mudanças ficam
// por gravar no editor, com as dos jogadores e dos clubes.

export default function StaffPanel({
  file,
  id,
  edit,
  onEdit,
  run,
}: {
  file: string
  id: number
  /** O que já se abriu (e mudou) desta pessoa nesta sessão do editor. */
  edit: StaffEdit | undefined
  onEdit: (e: StaffEdit) => void
  /** O `run` do editor: liga o "a trabalhar" e mostra o erro do core. */
  run: <T>(job: () => Promise<T>) => Promise<T | undefined>
}) {
  // Mudar de clube: a procura e o nome do clube escolhido
  const [clubQuery, setClubQuery] = useState('')
  const [clubHits, setClubHits] = useState<EditorHit[]>([])
  const [clubName, setClubName] = useState<string | null>(null)

  // Ao mudar de pessoa, ou depois de gravar (o editor esquece o que tinha aberto): abre de novo
  const missing = !edit
  useEffect(() => {
    setClubHits([])
    if (edit) {
      setClubName(edit.data.clubId === edit.staff.data.clubId ? edit.staff.club : null)
      return
    }
    let live = true
    api.editorStaff(file, id).then(
      (s) => {
        if (!live) return
        onEdit({ staff: s, data: s.data, draft: {} })
        setClubName(s.club)
      },
      () => {},
    )
    return () => {
      live = false
    }
    // O `edit` muda a cada tecla: conta só se falta
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file, id, missing])

  if (!edit) return null
  const { staff, data, draft } = edit
  const set = (p: Partial<EditorStaffData>) => onEdit({ ...edit, data: { ...data, ...p } })
  const setDraft = (key: string, v: number) => onEdit({ ...edit, draft: { ...draft, [key]: v } })

  const findClubs = async () => {
    const found = await run(() => api.editorSearchClubs(file, clubQuery))
    if (found) setClubHits(found)
  }

  const attrRow = (key: string) => {
    const a = staff.attributes.find((x) => x.key === key)
    return a ? <AttributeRow key={key} a={a} value={draft[key] ?? a.shown} onChange={(v) => setDraft(key, v)} /> : null
  }
  const staffKeys = staff.attributes
    .filter((a) => a.group === 'staff')
    .map((a) => a.key)
    .sort((x, y) => (LABELS[x] ?? x).localeCompare(LABELS[y] ?? y, 'pt'))
  const half = Math.ceil(staffKeys.length / 2)
  const mental = staff.attributes.filter((a) => a.group === 'mental').map((a) => a.key)

  return (
    <>
      <div className="mb-1.5 flex items-baseline gap-3">
        <span className="font-title text-[22px] text-white">{staff.name}</span>
        <span className="text-cm-magenta">{clubName ?? 'Sem clube'}</span>
        {staffDirty(edit) && <span className="text-[13px] text-cm-yellow">Alterações por salvar</span>}
      </div>
      <div className="grid grid-cols-3 gap-2">
        <div className={BOX}>
          <Header>Atributos de staff</Header>
          {staffKeys.slice(0, half).map(attrRow)}
        </div>
        <div className={BOX}>
          <Header>Atributos de staff</Header>
          {staffKeys.slice(half).map(attrRow)}
        </div>
        <div className={BOX}>
          <Header>Mentais</Header>
          {mental.map(attrRow)}
        </div>
      </div>
      <div className="mt-2 grid grid-cols-2 gap-2">
        <div className={BOX}>
          <Header>Ficha</Header>
          <NumField label="Capacidade atual (CA)" value={data.ability} max={200} width="w-20" onChange={(v) => set({ ability: v })} />
          <NumField label="Potencial (PA)" value={data.potential} max={200} width="w-20" onChange={(v) => set({ potential: v })} />
          <NumField
            label="Reputação local"
            value={data.homeReputation}
            max={10000}
            width="w-20"
            onChange={(v) => set({ homeReputation: v })}
          />
          <NumField
            label="Reputação atual"
            value={data.currentReputation}
            max={10000}
            width="w-20"
            onChange={(v) => set({ currentReputation: v })}
          />
          <NumField
            label="Reputação mundial"
            value={data.worldReputation}
            max={10000}
            width="w-20"
            onChange={(v) => set({ worldReputation: v })}
          />
        </div>
        <div className={BOX}>
          <Header>Clube e contrato</Header>
          <div className={ROW}>
            <span className="min-w-0 flex-1 truncate">Clube</span>
            <span className="truncate text-cm-magenta">{clubName ?? 'Sem clube'}</span>
          </div>
          <form
            className="flex gap-1 px-2 py-1"
            onSubmit={(e) => {
              e.preventDefault()
              void findClubs()
            }}
          >
            <input
              className="cm-field h-7 min-w-0 flex-1 text-[13px]"
              placeholder="Procurar outro clube"
              value={clubQuery}
              onChange={(e) => setClubQuery(e.target.value)}
            />
            <button className="cm-purple h-7 px-2 text-[13px]">Procurar</button>
            <button
              type="button"
              className="cm-purple h-7 px-2 text-[13px]"
              onClick={() => {
                set({ clubId: null, jobId: null })
                setClubName(null)
              }}
            >
              Livre
            </button>
          </form>
          {clubHits.length > 0 && (
            <div className="mx-2 max-h-28 overflow-auto rounded bg-black/40">
              {clubHits.map((c) => (
                <button
                  key={c.id}
                  className="block h-6 w-full truncate px-2 text-left text-[13px] text-cm-yellow hover:bg-cm-sel"
                  onClick={() => {
                    set({ clubId: c.id, jobId: data.jobId ?? staff.jobs[0]?.id ?? null })
                    setClubName(c.name)
                    setClubHits([])
                  }}
                >
                  {c.name}
                </button>
              ))}
            </div>
          )}
          <label className={ROW}>
            <span className="min-w-0 flex-1 truncate">Função</span>
            <select
              className="cm-field h-6 w-44 text-[13px]"
              disabled={data.clubId === null}
              value={data.jobId ?? ''}
              onChange={(e) => set({ jobId: Number(e.target.value) })}
            >
              {data.jobId === null && <option value="">-</option>}
              {staff.jobs.map((j) => (
                <option key={j.id} value={j.id}>
                  {j.label}
                </option>
              ))}
            </select>
          </label>
          <NumField label="Salário (por semana)" value={data.wage} width="w-28" onChange={(v) => set({ wage: v })} />
          <label className={ROW}>
            <span className="min-w-0 flex-1 truncate">Fim do contrato</span>
            <input
              className="cm-field h-6 w-32 text-center text-[13px]"
              placeholder="AAAA-MM-DD"
              value={data.contractExpires ?? ''}
              onChange={(e) => set({ contractExpires: e.target.value || null })}
            />
          </label>
        </div>
      </div>
    </>
  )
}
