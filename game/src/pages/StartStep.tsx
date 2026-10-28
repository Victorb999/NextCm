import type { StartSeason } from '../bindings/StartSeason'
import { DialogFrame, Overlay } from '../components/Dialog'

// "Escolher Iniciar Época", como no CM: a época europeia de 2026/27 (1/7/2026, com o Brasil a
// meio de 2026) ou a de 2026 do Brasil (1/1/2026, depois de jogar o 2º semestre de 2025).

const OPTIONS: [StartSeason, string, string][] = [
  ['europe', 'Europa 26/27', 'Começa a 1.7.2026, com o Brasileirão de 2026 a meio.'],
  ['brazil', 'Brasil 2026', 'Começa a 1.1.2026, com a Europa a meio de 2025/26: o 2º semestre de 2025 joga-se ao criar o jogo (demora um pouco).'],
]

export default function StartStep({
  value,
  onChange,
  brazil,
  onCancel,
  onNext,
}: {
  value: StartSeason
  onChange: (s: StartSeason) => void
  /** Há uma liga de ano civil escolhida (o Brasil 2026 só faz sentido com ela). */
  brazil: boolean
  onCancel: () => void
  onNext: () => void
}) {
  const options = OPTIONS.filter(([k]) => k !== 'brazil' || brazil)
  const detail = options.find(([k]) => k === value)?.[2]
  return (
    <Overlay>
      <DialogFrame title="Escolher Iniciar Época" onCancel={onCancel} cancelLabel="Atrás" onOk={onNext} okLabel="Seguinte">
        <div className="grid grid-cols-2 gap-0 py-2">
          {options.map(([k, label]) => (
            <button
              key={k}
              onClick={() => onChange(k)}
              onDoubleClick={onNext}
              className={`h-16 border-2 bg-black/70 font-title text-[20px] ${
                value === k ? 'border-cm-yellow text-cm-yellow' : 'border-[#1a1aa8] text-white hover:text-cm-yellow'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        {detail && <div className="pt-1 text-[13px] text-[#bfe6c4]">{detail}</div>}
      </DialogFrame>
    </Overlay>
  )
}
