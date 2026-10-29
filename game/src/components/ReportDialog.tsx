import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ReportStaff } from '../bindings/ReportStaff'
import { DialogFrame, Overlay } from './Dialog'
import { Empty } from './Screen'

// "Pedir Relatório Médico" / "Pedir Relatório Preparador Físico": escolher quem do staff faz o
// relatório (amarelo = escolhido, roxo = não), como no CM. As mensagens chegam já.

export default function ReportDialog({
  kind,
  personId,
  name,
  onClose,
  onDone,
}: {
  kind: 'medical' | 'fitness'
  personId: number
  name: string
  onClose: () => void
  onDone: (text: string) => void
}) {
  const [staff, setStaff] = useState<ReportStaff[]>([])
  const [chosen, setChosen] = useState<Set<number>>(new Set())
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.reportStaff(kind).then(
      (list) => {
        setStaff(list)
        // Começa com o primeiro (o melhor, ou o adjunto)
        setChosen(new Set(list.slice(0, 1).map((s) => s.id)))
      },
      (e) => setError(String(e)),
    )
  }, [kind])

  const toggle = (id: number) => {
    const next = new Set(chosen)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    setChosen(next)
  }
  const confirm = async () => {
    try {
      const n = await api.staffReport(personId, kind, [...chosen])
      onDone(n === 1 ? 'O relatório chegou às mensagens.' : `Os ${n} relatórios chegaram às mensagens.`)
    } catch (e) {
      setError(String(e))
    }
  }
  const title = kind === 'medical' ? 'Pedir Relatório Médico' : 'Pedir Relatório Preparador Físico'
  const who = kind === 'medical' ? 'o fisioterapeuta' : 'o preparador físico'

  return (
    <Overlay>
      <DialogFrame title={title} width={600} cancelLabel="Cancelar" okLabel="Confirmar" onCancel={onClose} onOk={() => void confirm()}>
        <div className="flex items-start justify-between gap-2 pb-2">
          <div className="font-title text-[18px] text-white">
            Especifique {who} que prefere para elaborar um relatório sobre {name}:
          </div>
          <button className="cm-purple h-6 shrink-0 px-3" onClick={() => setChosen(new Set(staff.map((s) => s.id)))}>
            Escolher Todos
          </button>
        </div>
        {staff.length === 0 && !error && <Empty className="py-4">O clube não tem ninguém para este relatório.</Empty>}
        {staff.map((s) => {
          const on = chosen.has(s.id)
          return (
            <button key={s.id} onClick={() => toggle(s.id)} className="flex w-full justify-between px-6 py-0.5 text-left font-title text-[18px]">
              <span className={on ? 'text-cm-yellow' : 'text-[#7b3fd1]'}>{s.name}</span>
              <span className={on ? 'text-cm-yellow' : 'text-[#7b3fd1]'}>{s.job}</span>
            </button>
          )
        })}
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
