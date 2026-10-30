import { DialogFrame, Overlay } from '../components/Dialog'
import { useGame } from '../game'

/** A direção despediu o treinador: fica sem clube e procura outro (Informação de Empregos). */
export default function SackedDialog({ onRead, onJobs }: { onRead: () => void; onJobs: () => void }) {
  const game = useGame()
  return (
    <Overlay>
      <DialogFrame title="Despedido" width={560}>
        <div className="py-3 text-center text-[16px] leading-snug text-white">
          A Direcção do {game.info?.club.name} dispensou-o do cargo de treinador. Procure um novo clube.
        </div>
        <div className="flex gap-1 pb-1">
          <button className="cm-purple h-8 flex-1 font-title text-[16px]" onClick={onRead}>
            Ver Notícias
          </button>
          <button className="cm-purple h-8 flex-1 font-title text-[16px]" onClick={onJobs}>
            Informação de Empregos
          </button>
        </div>
      </DialogFrame>
    </Overlay>
  )
}
