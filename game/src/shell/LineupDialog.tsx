import type { LineupCheck } from '../bindings/LineupCheck'
import { DialogFrame, Overlay } from '../components/Dialog'

/**
 * Antes do dia de jogo, como no CM: "Escolha Equipa" quando falta o onze (Ok vai às Tácticas)
 * e "Confirmar Por Favor" com os avisos (Não fica, Sim avança para o jogo).
 */
export default function LineupDialog({
  check,
  onTactics,
  onCancel,
  onGo,
}: {
  check: LineupCheck
  onTactics: () => void
  onCancel: () => void
  onGo: () => void
}) {
  const text = 'whitespace-pre-line py-1 font-title text-[19px] leading-snug text-white'
  if (check.problem)
    return (
      <Overlay>
        <DialogFrame title="Escolha Equipa" width={620}>
          <div className={`${text} py-3`}>{check.problem}</div>
          <button className="cm-purple h-9 w-full font-title text-xl" onClick={onTactics}>
            Ok
          </button>
        </DialogFrame>
      </Overlay>
    )
  return (
    <Overlay>
      <DialogFrame title="Confirmar Por Favor" width={620} onCancel={onCancel} onOk={onGo} cancelLabel="Não" okLabel="Sim">
        {check.warnings.map((w) => (
          <div key={w} className={`${text} pb-3`}>
            {w}
          </div>
        ))}
        <div className={text}>Avançar para o jogo ?</div>
      </DialogFrame>
    </Overlay>
  )
}
