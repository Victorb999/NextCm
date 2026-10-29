import { Mic } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { PressView } from '../bindings/PressView'
import { BottomBar, ErrorBox, ScreenHeader } from '../components/Screen'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

/**
 * Conferência de imprensa: as perguntas dos jornalistas uma de cada vez, com as respostas
 * em botões; no fim, as frases vão para as notícias. "Mandar o Adjunto" responde por ele.
 */
export default function PressPage({ id }: { id: number }) {
  const nav = useNav()
  const game = useGame()
  useEscBack()
  const [view, setView] = useState<PressView | null>(null)
  const [answers, setAnswers] = useState<number[]>([])
  const [result, setResult] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.pressView(id).then(setView, (e) => setError(String(e)))
  }, [id])

  const finish = async (f: () => Promise<string>) => {
    try {
      setResult(await f())
      setError(null)
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    }
  }
  const pick = (i: number) => {
    if (!view) return
    const next = [...answers, i]
    setAnswers(next)
    if (next.length === view.questions.length) void finish(() => api.answerPress(id, next))
  }

  const q = view && view.status === 'pending' && !result ? view.questions[answers.length] : null
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Mic size={22} />} title={view?.title ?? 'Conferência de Imprensa'} />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/70 px-6 py-4">
        <ErrorBox className="mb-2">{error}</ErrorBox>
        {view && view.status !== 'pending' && !result && (
          <div className="font-title text-[19px] text-white">Esta conferência de imprensa já acabou.</div>
        )}
        {q && (
          <>
            <div className="text-[15px] text-cm-magenta">
              {view!.intro} Pergunta {answers.length + 1} de {view!.questions.length}
            </div>
            <div className="py-4 font-title text-[24px] font-bold leading-snug text-cm-yellow">{q.text}</div>
            <div className="flex flex-col gap-1.5">
              {q.answers.map((a, i) => (
                <button key={a} className="cm-purple min-h-9 px-4 py-1 text-left font-title text-[18px]" onClick={() => pick(i)}>
                  {a}
                </button>
              ))}
            </div>
            <div className="mt-auto flex justify-end pt-4">
              <button className="cm-purple h-8 w-56" onClick={() => void finish(() => api.skipPress(id))}>
                Mandar o Adjunto
              </button>
            </div>
          </>
        )}
        {view && !q && !result && view.status === 'pending' && <div className="font-title text-[19px] text-cm-magenta">A falar com os jornalistas…</div>}
        {result && <div className="whitespace-pre-line font-title text-[19px] leading-snug text-white">{result}</div>}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
