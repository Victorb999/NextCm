import { useEffect, useState } from 'react'
import { api } from '../api'
import type { SquadRoleView } from '../bindings/SquadRoleView'
import { DialogFrame, Overlay } from './Dialog'

/**
 * "Definir Função no Plantel" (o "Set Squad Status" do FM): a função do jogador sem mexer no
 * contrato. As funções e as que a idade não deixa vêm do core; a moral também é com ele.
 */
export default function SquadRoleDialog({
  personId,
  onCancel,
  onDone,
}: {
  personId: number
  onCancel: () => void
  onDone: (message: string) => void
}) {
  const [view, setView] = useState<SquadRoleView | null>(null)
  const [chosen, setChosen] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    api.squadRoleView(personId).then(
      (v) => {
        setView(v)
        setChosen(v.current)
      },
      (e) => setError(String(e)),
    )
  }, [personId])
  const ok = () => {
    if (!chosen || chosen === view?.current) return onCancel()
    api.setSquadRole(personId, chosen).then(onDone, (e) => setError(String(e)))
  }
  return (
    <Overlay>
      <DialogFrame title="Definir Função no Plantel" width={620} onCancel={onCancel} onOk={chosen ? ok : undefined} cancelLabel="Atrás">
        {view && <div className="pb-2 text-[17px] text-white">A função de {view.name} no plantel:</div>}
        <div className="cm-zebra overflow-hidden rounded bg-black/30">
          {view?.roles.map((r) => (
            <button
              key={r.key}
              disabled={!r.allowed}
              title={r.veto ?? undefined}
              onClick={() => setChosen(r.key)}
              className={`flex h-8 w-full items-center gap-3 px-3 text-left text-[15px] disabled:text-white/30 ${
                chosen === r.key ? '!bg-cm-sel text-white' : 'text-[#e3e1e9] enabled:hover:text-cm-yellow'
              }`}
            >
              <span className={`size-3 shrink-0 rounded-full border ${chosen === r.key ? 'border-cm-yellow bg-cm-yellow' : 'border-white/50'}`} />
              <span className="flex-1">{r.label}</span>
              {r.key === view.current && <span className="text-[12px] font-bold text-cm-green">Agora</span>}
            </button>
          ))}
        </div>
        {view && !view.current && <div className="pt-2 text-[13px] text-[#bfe6c4]">Sem função definida: conta o lugar no plantel.</div>}
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
