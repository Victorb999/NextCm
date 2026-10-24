import { useEffect, useState } from 'react'
import { api } from '../api'
import type { DisciplineOptions } from '../bindings/DisciplineOptions'
import { DialogFrame, Overlay } from './Dialog'

// "Disciplinar Jogador", como no CM: o castigo com − / + e a razão (as que não aconteceram
// ficam a roxo). A reação do jogador vem do core.

export default function DisciplineDialog({
  personId,
  name,
  onClose,
  onDone,
}: {
  personId: number
  name: string
  onClose: () => void
  onDone: (text: string) => void
}) {
  const [options, setOptions] = useState<DisciplineOptions | null>(null)
  const [action, setAction] = useState(0)
  const [reason, setReason] = useState('none')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.disciplineOptions(personId).then(setOptions, (e) => setError(String(e)))
  }, [personId])

  const confirm = async () => {
    try {
      onDone(await api.discipline(personId, action, reason))
    } catch (e) {
      setError(String(e))
    }
  }
  const last = (options?.actions.length ?? 1) - 1

  return (
    <Overlay>
      <DialogFrame title="Disciplinar Jogador" width={620} cancelLabel="Cancelar" okLabel="Confirmar" onCancel={onClose} onOk={() => void confirm()}>
        <div className="pb-4 font-title text-[19px] text-white">Especifique os detalhes da acção disciplinar sobre {name}</div>
        <div className="flex items-center gap-3 pb-4 pl-4 font-title text-[19px] text-white">
          <span className="w-56">Acção disciplinar:</span>
          <button className="h-6 w-6 bg-black text-white disabled:text-[#555]" disabled={action === 0} onClick={() => setAction(action - 1)}>
            −
          </button>
          <span className="w-80 text-center text-cm-yellow">{options?.actions[action] ?? ''}</span>
          <button className="h-6 w-6 bg-black text-white disabled:text-[#555]" disabled={action >= last} onClick={() => setAction(action + 1)}>
            +
          </button>
        </div>
        <div className="pb-2 pl-4 font-title text-[19px] text-white">Razão Para Acção Disciplinar</div>
        {options?.reasons.map((r) => (
          <button
            key={r.key}
            disabled={!r.available}
            title={r.reason ?? undefined}
            onClick={() => setReason(r.key)}
            className={`block py-0.5 pl-12 text-left font-title text-[19px] ${
              !r.available ? 'cursor-default text-[#7b3fd1]' : reason === r.key ? 'text-cm-yellow' : 'text-white hover:text-cm-yellow'
            }`}
          >
            {r.label}
          </button>
        ))}
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
