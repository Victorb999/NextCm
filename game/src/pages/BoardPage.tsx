import { useEffect, useState } from 'react'
import { api } from '../api'
import type { BoardConfidence } from '../bindings/BoardConfidence'
import { BottomBar, TitleBand } from '../components/Screen'
import { useEscBack, useNav } from '../nav'

// "Confiança da Direcção" (menu do treinador): o que a direção pensa do treinador, das finanças,
// dos adeptos e dos jogadores valiosos, em parágrafos como no CM. Os pedidos à direção e à liga
// chegam nos passos seguintes do 3.11.

export default function BoardPage() {
  const nav = useNav()
  useEscBack()
  const [view, setView] = useState<BoardConfidence | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.boardConfidence().then(setView, (e) => setError(String(e)))
  }, [])

  const fg = view?.club.fg ?? '#000000'
  const bg = view?.club.bg ?? '#ffffff'
  return (
    <div className="flex h-full flex-col">
      <TitleBand title={view?.club.name ?? ''} fg={fg} bg={bg} />
      <div className="py-2 text-center font-title text-[30px] font-bold text-cm-yellow">Confiança da Direcção</div>
      <div className="flex shrink-0 gap-1 px-3">
        <button className="cm-purple h-6 w-40" disabled title="Chega no próximo passo">
          Pedido à Direcção
        </button>
        <button className="cm-purple h-6 w-40" disabled title="Chega no próximo passo">
          Pedido à Liga
        </button>
      </div>
      <div className="mx-1.5 mt-1.5 min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/80 px-4 py-3">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        {view?.paragraphs.map((p) => (
          <p key={p} className="mb-5 font-title text-[22px] leading-snug text-white">
            {p}
          </p>
        ))}
      </div>
      <BottomBar onLeft={nav.back} />
    </div>
  )
}
