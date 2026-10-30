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
        <div className="pb-3 text-[15px] text-white">Especifique os detalhes da acção disciplinar sobre {name}.</div>
        <div className="flex items-center gap-2 pb-3">
          <span className="w-44 text-[15px] font-semibold text-white">Acção disciplinar</span>
          <button className="cm-purple h-7 w-8 px-0" disabled={action === 0} onClick={() => setAction(action - 1)}>
            −
          </button>
          <span className="flex h-7 flex-1 items-center justify-center rounded bg-black/50 text-[15px] font-bold text-cm-yellow">
            {options?.actions[action] ?? ''}
          </span>
          <button className="cm-purple h-7 w-8 px-0" disabled={action >= last} onClick={() => setAction(action + 1)}>
            +
          </button>
        </div>
        <div className="pb-1 font-mono text-[11px] font-bold uppercase tracking-[0.12em] text-cm-orange">Razão para a acção disciplinar</div>
        <div className="cm-zebra overflow-hidden rounded bg-black/30">
          {options?.reasons.map((r) => (
            <button
              key={r.key}
              disabled={!r.available}
              onClick={() => setReason(r.key)}
              className={`flex min-h-7 w-full items-center gap-2 px-3 text-left text-[14px] ${
                !r.available ? 'cursor-default text-[#8b6fc4]' : reason === r.key ? '!bg-cm-sel font-bold text-white' : 'text-white hover:text-cm-yellow'
              }`}
            >
              <span className={`size-3 shrink-0 rounded-full border ${reason === r.key ? 'border-cm-yellow bg-cm-yellow' : 'border-current'}`} />
              <span className="flex-1">{r.label}</span>
              {/* Porque não se pode escolher (não aconteceu), como o roxo do CM */}
              {!r.available && r.reason && <span className="text-[12px] text-[#8b6fc4]">{r.reason}</span>}
            </button>
          ))}
        </div>
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
