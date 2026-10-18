import { DialogFrame, Overlay } from '../components/Dialog'
import { useGame } from '../game'

/** Menu inicial, quando não há jogo aberto. */
export default function StartScreen({ onOptions }: { onOptions: () => void }) {
  const game = useGame()
  const item = (label: string, onClick: () => void, hint: string) => (
    <button
      onClick={onClick}
      className="cm-purple flex h-14 w-full flex-col items-center justify-center font-title text-[22px] leading-tight"
    >
      {label}
      <span className="font-sans text-[12px] text-[#d8c8f4]">{hint}</span>
    </button>
  )
  return (
    <Overlay>
      <DialogFrame title="NextCM" width={460}>
        <div className="space-y-2 py-2">
          {item('Iniciar Novo Jogo', () => game.setMode('newGame'), 'Escolha o seu clube no mundo do patch CLZ 25/26')}
          {item('Carregar Jogo', () => game.setMode('loadGame'), 'Continue um jogo gravado')}
          {item('Explorar Base de Dados', () => game.setMode('explore'), 'Navegue por jogadores, clubes e competições')}
          {item('Opções', onOptions, 'Moeda, salários, cores e fundo')}
        </div>
      </DialogFrame>
    </Overlay>
  )
}
