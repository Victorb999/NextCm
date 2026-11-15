import { useState } from 'react'
import { api } from '../api'
import type { StaffHireView } from '../bindings/StaffHireView'
import { money, wage as wageText, wagePeriod } from '../format'
import { DialogFrame, Overlay } from './Dialog'
import MoneySelect from './MoneySelect'

// Oferecer Contrato a um membro do staff (4.0, item 24), como no CM: a função (com quantos o clube
// tem e o máximo da direcção), o salário semanal, os anos e a multa ao clube dele. A resposta
// chega amanhã às mensagens; o core valida tudo.

export default function StaffContractDialog({
  view,
  onCancel,
  onDone,
}: {
  view: StaffHireView
  onCancel: () => void
  onDone: (message: string) => void
}) {
  const period = wagePeriod()
  // Por omissão a função de agora (se é das que se oferecem), senão a primeira com lugar
  const start = view.jobs.find((j) => j.label === view.job) ?? view.jobs.find((j) => j.count < j.max) ?? view.jobs[0]
  const [job, setJob] = useState(start?.id ?? 6)
  const current = view.jobs.find((j) => j.id === job)
  const [w, setW] = useState(current?.asked ?? 0)
  const [years, setYears] = useState(2)
  const [error, setError] = useState<string | null>(null)

  const pickJob = (id: number) => {
    setJob(id)
    setW(view.jobs.find((j) => j.id === id)?.asked ?? w)
  }
  const offer = async () => {
    try {
      onDone(await api.offerStaffContract(view.personId, job, Math.round(w), years))
    } catch (e) {
      setError(String(e))
    }
  }

  const row = 'flex h-9 items-center gap-2 px-4'
  const label = 'w-44 shrink-0 text-[15px] font-semibold text-white'
  return (
    <Overlay>
      <DialogFrame
        title={view.mine ? 'Renovar Contrato' : 'Oferecer Contrato'}
        width={620}
        okLabel="Oferecer"
        onCancel={onCancel}
        onOk={() => void offer()}
      >
        <div className="py-2">
          <div className="px-4 pb-2 font-title text-[19px] text-white">
            {view.name}
            {view.club && (
              <span className="text-cm-muted">
                {' '}
                ({view.job ?? 'Staff'}, {view.club.shortName})
              </span>
            )}
          </div>
          <div className="cm-zebra">
            <div className={row}>
              <span className={label}>Função</span>
              <select
                value={job}
                onChange={(e) => pickJob(Number(e.target.value))}
                className="h-7 w-72 rounded border border-cm-dlgline bg-black px-2 text-[15px] text-cm-yellow outline-none focus:border-cm-yellow"
              >
                {view.jobs.map((j) => (
                  <option key={j.id} value={j.id}>
                    {j.label} ({j.count} de {j.max})
                  </option>
                ))}
              </select>
            </div>
            <div className={row}>
              <span className={label}>Salário</span>
              <MoneySelect value={w} factor={period.factor} onChange={setW} />
              <button className="cm-purple h-7 w-8 px-0" title="O que ele pede" onClick={() => setW(current?.asked ?? w)}>
                =
              </button>
            </div>
            <div className={row}>
              <span className={label}>Duração</span>
              <button className="cm-purple h-7 w-8 px-0" disabled={years <= 1} onClick={() => setYears(years - 1)}>
                −
              </button>
              <span className="w-44 text-center text-[15px] text-cm-yellow">
                {years} ano{years > 1 ? 's' : ''}
              </span>
              <button className="cm-purple h-7 w-8 px-0" disabled={years >= 5} onClick={() => setYears(years + 1)}>
                +
              </button>
            </div>
            {view.fee > 0 && (
              <div className={row}>
                <span className={label}>Multa</span>
                <span className="text-[15px] text-cm-yellow">
                  {money(view.fee)} ao {view.club?.shortName}
                </span>
              </div>
            )}
          </div>
          <div className="px-4 pt-2 text-[14px] text-cm-muted">
            {current && `Para ${current.label}, ${view.name} pede cerca de ${wageText(current.asked, true)}.`}
            {view.mine && view.wage > 0 && ` Hoje ganha ${wageText(view.wage, true)}.`}
          </div>
          {error && <div className="px-4 pt-2 text-[15px] font-bold text-cm-red">{error}</div>}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
