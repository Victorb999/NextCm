import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TalkReaction } from '../bindings/TalkReaction'
import type { TalkView } from '../bindings/TalkView'
import { BottomBar, TitleBand } from './Screen'

// Palestra (antes do jogo, ao intervalo, no fim): as frases do treinador, a sugestão do
// adjunto e, depois de falar, a reação de cada jogador. As regras estão no core.

const COLOURS: Record<string, string> = {
  Motivado: 'text-[#4cd964]',
  Focado: 'text-[#a8e6a1]',
  'Sem reação': 'text-white',
  Complacente: 'text-cm-orange',
  Nervoso: 'text-cm-orange',
  Irritado: 'text-[#ff5050]',
}

export default function TalkPanel({ onClose }: { onClose: () => void }) {
  const [view, setView] = useState<TalkView | null>(null)
  const [reactions, setReactions] = useState<TalkReaction[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    api.talkView().then(setView, (e) => setError(String(e)))
  }, [])
  const talk = async (key: string) => {
    try {
      setReactions(await api.giveTalk(key))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }
  return (
    <div className="flex h-full flex-col">
      <TitleBand title={view?.title ?? 'Palestra'} fg="#000000" bg="#f4f4f4" />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/70 px-6 py-3">
        {error && <div className="mb-2 bg-red-900 px-2 py-1 text-white">{error}</div>}
        {view && !reactions && (view.done ? (
          <div className="font-title text-[19px] text-white">Já falou à equipa neste momento.</div>
        ) : (
          <>
            <div className="pb-2 font-title text-[19px] text-white">O que vai dizer à equipa?</div>
            <div className="flex flex-col gap-1.5">
              {view.options.map((o) => (
                <button key={o.key} className="cm-purple min-h-9 px-4 py-1 text-left font-title text-[18px]" onClick={() => void talk(o.key)}>
                  {o.label}
                  {o.key === view.suggested && <span className="ml-3 text-[13px] text-cm-yellow">(sugestão do adjunto)</span>}
                </button>
              ))}
            </div>
          </>
        ))}
        {reactions && (
          <div className="min-h-0 flex-1 overflow-auto">
            {reactions.map((r) => (
              <div key={r.personId} className="flex h-[22px] items-center justify-between border-b border-cm-line px-2 text-[15px]">
                <span className="text-white">{r.name}</span>
                <span className={`font-bold ${COLOURS[r.reaction] ?? 'text-white'}`}>{r.reaction}</span>
              </div>
            ))}
          </div>
        )}
      </div>
      <BottomBar left="Atrás" onLeft={onClose} />
    </div>
  )
}
