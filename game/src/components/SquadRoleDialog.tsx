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
  const [job, setJob] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    api.squadRoleView(personId).then(
      (v) => {
        setView(v)
        setChosen(v.current)
        setJob(v.job)
      },
      (e) => setError(String(e)),
    )
  }, [personId])
  const ok = async () => {
    const calls: string[] = []
    if (chosen && chosen !== view?.current) calls.push(chosen)
    // A função no clube (Jogador/Treinador...) vai pelo mesmo comando, com "job:"
    if (job && job !== view?.job) calls.push(`job:${job}`)
    if (!calls.length) return onCancel()
    try {
      const said: string[] = []
      for (const c of calls) said.push(await api.setSquadRole(personId, c))
      onDone(said.join(' '))
    } catch (e) {
      setError(String(e))
    }
  }
  return (
    <Overlay>
      <DialogFrame
        title="Definir Função no Plantel"
        width={620}
        onCancel={onCancel}
        onOk={chosen || job ? ok : undefined}
        cancelLabel="Atrás"
      >
        {view && <div className="pb-2 text-[17px] text-white">A função de {view.name} no plantel:</div>}
        <div className="cm-zebra overflow-hidden rounded bg-black/30">
          {view?.roles.map((r) => (
            <button
              key={r.key}
              disabled={!r.allowed}
              title={r.veto ?? undefined}
              onClick={() => setChosen(r.key)}
              className={`flex h-8 w-full items-center gap-3 px-3 text-left text-[15px] disabled:text-white/30 ${
                chosen === r.key ? '!bg-cm-sel text-white' : 'text-cm-text enabled:hover:text-cm-yellow'
              }`}
            >
              <span
                className={`size-3 shrink-0 rounded-full border ${chosen === r.key ? 'border-cm-yellow bg-cm-yellow' : 'border-white/50'}`}
              />
              <span className="flex-1">{r.label}</span>
              {r.key === view.current && <span className="text-[12px] font-bold text-cm-green">Agora</span>}
            </button>
          ))}
        </div>
        {view && !view.current && <div className="pt-2 text-[13px] text-cm-dlgtext">Sem função definida: conta o lugar no plantel.</div>}
        {view && view.jobs.length > 0 && (
          <>
            <div className="pb-2 pt-3 text-[17px] text-white">Função no clube:</div>
            <div className="cm-zebra overflow-hidden rounded bg-black/30">
              {view.jobs.map((r) => (
                <button
                  key={r.key}
                  onClick={() => setJob(r.key)}
                  className={`flex h-8 w-full items-center gap-3 px-3 text-left text-[15px] ${
                    job === r.key ? '!bg-cm-sel text-white' : 'text-cm-text hover:text-cm-yellow'
                  }`}
                >
                  <span
                    className={`size-3 shrink-0 rounded-full border ${job === r.key ? 'border-cm-yellow bg-cm-yellow' : 'border-white/50'}`}
                  />
                  <span className="flex-1">{r.label}</span>
                  {r.key === view.job && <span className="text-[12px] font-bold text-cm-green">Agora</span>}
                </button>
              ))}
            </div>
          </>
        )}
        {error && <div className="pt-2 text-[15px] font-bold text-[#ffb0a0]">{error}</div>}
      </DialogFrame>
    </Overlay>
  )
}
