import { Mic } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { PressView } from '../bindings/PressView'
import { BottomBar, Empty, ErrorBox, Panel, ScreenHeader } from '../components/Screen'
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
      <ScreenHeader
        icon={<Mic size={22} />}
        eyebrow={q ? `Pergunta ${answers.length + 1} de ${view!.questions.length}` : undefined}
        title={view?.title ?? 'Conferência de Imprensa'}
        right={
          q && (
            <button className="cm-purple h-8 px-4" onClick={() => void finish(() => api.skipPress(id))}>
              Mandar o Adjunto
            </button>
          )
        }
      />
      <div className="mx-2 flex min-h-0 flex-1 flex-col gap-2">
        <ErrorBox>{error}</ErrorBox>
        {view && view.status !== 'pending' && !result && <Empty>Esta conferência de imprensa já acabou.</Empty>}
        {q && (
          <>
            <div className="shrink-0 rounded-md bg-[#1a1b21] px-4 py-3">
              <div className="text-[13px] text-cm-magenta">{view!.intro}</div>
              <div className="pt-1 font-title text-[20px] font-bold leading-snug text-cm-yellow">{q.text}</div>
            </div>
            <Panel title="Resposta" className="shrink-0">
              <div className="cm-zebra">
                {q.answers.map((a, i) => (
                  <button key={a} className="flex min-h-9 w-full items-center gap-3 px-4 py-1 text-left text-[15px] text-white hover:!bg-cm-sel" onClick={() => pick(i)}>
                    <span className="w-5 shrink-0 font-mono text-[12px] font-bold text-cm-orange">{i + 1}.</span>
                    {a}
                  </button>
                ))}
              </div>
            </Panel>
          </>
        )}
        {view && !q && !result && view.status === 'pending' && <Empty>A falar com os jornalistas…</Empty>}
        {result && (
          <Panel title="O que saiu na imprensa" className="shrink-0">
            <div className="whitespace-pre-line px-4 py-3 text-[15px] leading-relaxed text-white">{result}</div>
          </Panel>
        )}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
