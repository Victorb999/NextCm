import { DialogFrame } from '../components/Dialog'
import { useGame } from '../game'

/** Menu inicial, quando não há jogo aberto: a faixa com o logo e, por baixo, o diálogo verde. */
export default function StartScreen({ onOptions }: { onOptions: () => void }) {
  const game = useGame()
  const item = (label: string, onClick: () => void, hint: string) => (
    <button
      onClick={onClick}
      className="bg-[#01010199] flex h-14 rounded hover:bg-[#010101cc] transition
      w-full flex-col items-center justify-center font-title text-[22px] leading-tight"
    >
      {label}
      <span className="font-sans text-[12px] text-[#666]">{hint}</span>
    </button>
  )
  return (
    <div className="absolute inset-0 z-[60] flex flex-col bg-black/50">
      <div className="m-2 flex h-36 shrink-0 items-center justify-center">
        <img src="/logo-tela.svg" alt="NextCM" className="h-32" />
      </div>
      <div className="flex min-h-0 flex-1 items-center justify-center p-4 pt-0">
        <DialogFrame title="Preparar Jogo" width={460}>
          <div className="space-y-2 py-2">
            {item('Iniciar Novo Jogo', () => game.setMode('newGame'), 'Escolha o seu clube no mundo do patch CLZ 25/26')}
            {item('Carregar Jogo', () => game.setMode('loadGame'), 'Continue um jogo gravado')}
            {item('Explorar Base de Dados', () => game.setMode('explore'), 'Navegue por jogadores, clubes e competições')}
            {item('Editor de Jogos Salvos', () => game.setMode('editor'), 'Mude os atributos de um jogador de um jogo gravado')}
            {item('Opções', onOptions, 'Moeda, salários, cores e fundo')}
          </div>
          <div className="text-right text-[11px] text-[#666]">versão {__APP_VERSION__}</div>
        </DialogFrame>
      </div>
    </div>
  )
}
