import { useState } from 'react'
import { api } from '../api'
import type { OfferView } from '../bindings/OfferView'
import { wage as wageText, wagePeriod } from '../format'
import { DialogFrame, Overlay } from './Dialog'
import MoneySelect from './MoneySelect'

// Negociar a renovação do treinador com a Diretoria: o salário e as temporadas que ele pede.
// A resposta vem na hora (aceita, contrapõe ou encerra); o core decide tudo.

export default function RenewalDialog({
  offer,
  onCancel,
  onDone,
}: {
  offer: OfferView
  onCancel: () => void
  onDone: (o: OfferView) => void
}) {
  const period = wagePeriod()
  const [w, setW] = useState(offer.fee)
  const [years, setYears] = useState(Math.max(1, offer.years))
  const [error, setError] = useState<string | null>(null)

  const ask = async () => {
    try {
      onDone(await api.askRenewal(offer.id, Math.round(w), years))
    } catch (e) {
      setError(String(e))
    }
  }

  const row = 'flex h-9 items-center gap-2 px-4'
  const label = 'w-44 shrink-0 text-[15px] font-semibold text-white'
  return (
    <Overlay>
      <DialogFrame title="Negociar Contrato" width={620} okLabel="Pedir" onCancel={onCancel} onOk={() => void ask()}>
        <div className="py-2">
          <div className="px-4 pb-2 font-title text-[19px] text-white">
            {offer.name} <span className="text-cm-muted">(Treinador, {offer.buyer.shortName})</span>
          </div>
          <div className="cm-zebra">
            <div className={row}>
              <span className={label}>Salário</span>
              <MoneySelect value={w} factor={period.factor} onChange={setW} />
              <button className="cm-purple h-7 w-8 px-0" title="O que a Diretoria oferece" onClick={() => setW(offer.fee)}>
                =
              </button>
            </div>
            <div className={row}>
              <span className={label}>Duração</span>
              <button className="cm-purple h-7 w-8 px-0" disabled={years <= 1} onClick={() => setYears(years - 1)}>
                −
              </button>
              <span className="w-44 text-center text-[15px] text-cm-yellow">
                {years} temporada{years > 1 ? 's' : ''}
              </span>
              <button className="cm-purple h-7 w-8 px-0" disabled={years >= 5} onClick={() => setYears(years + 1)}>
                +
              </button>
            </div>
          </div>
          <div className="px-4 pt-2 text-[14px] text-cm-muted">
            A Diretoria oferece {wageText(offer.fee, true)} por {offer.years} temporada{offer.years > 1 ? 's' : ''}. Se pedir demais, ela
            contrapõe; insistir pode encerrar a negociação.
          </div>
          {error && <div className="px-4 pt-2 text-[15px] font-bold text-cm-red">{error}</div>}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
