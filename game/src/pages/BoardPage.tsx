import { useEffect, useState } from 'react'
import { api } from '../api'
import type { BoardConfidence } from '../bindings/BoardConfidence'
import type { BoardRequest } from '../bindings/BoardRequest'
import { BottomBar, TitleBand } from '../components/Screen'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// "Confiança da Direcção" (menu do treinador): o que a direção pensa do treinador, das finanças,
// dos adeptos e dos jogadores valiosos, em parágrafos como no CM. "Pedido à Direcção" abre a
// Reunião com a Direcção (as regras e as respostas vêm do core). O pedido à liga chega no
// passo seguinte do 3.11.

function Meeting({ onClose }: { onClose: (answer: string | null) => void }) {
  const game = useGame()
  const [requests, setRequests] = useState<BoardRequest[]>([])
  const [chosen, setChosen] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.boardRequests().then(setRequests, (e) => setError(String(e)))
  }, [])

  const confirm = async () => {
    if (!chosen) return
    try {
      const answer = await api.boardRequest(chosen)
      const info = await api.gameInfo()
      if (info) game.update(info)
      onClose(answer)
    } catch (e) {
      setError(String(e))
    }
  }
  const reason = requests.find((r) => r.key === chosen)?.reason

  return (
    <div className="flex h-full flex-col">
      <TitleBand title={game.info?.managerName ?? ''} fg="#000000" bg="#ffffff" />
      <div className="py-2 text-center font-title text-[30px] font-bold text-cm-yellow">Reunião com a Direcção</div>
      <div className="mx-1.5 min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/80 px-6 py-4">
        <div className="pb-3 font-title text-[20px] text-white">Especificar os detalhes das exigências à direcção:</div>
        {requests.map((r) => (
          <button
            key={r.key}
            disabled={!r.available}
            title={r.reason ?? undefined}
            onClick={() => setChosen(r.key)}
            className={`block py-0.5 pl-14 text-left font-title text-[20px] ${
              !r.available ? 'cursor-default text-[#7b3fd1]' : chosen === r.key ? 'text-cm-yellow' : 'text-white hover:text-cm-yellow'
            }`}
          >
            {r.label}
          </button>
        ))}
        {reason && <div className="pt-3 text-cm-magenta">{reason}</div>}
        {requests.some((r) => !r.available) && !reason && (
          <div className="pt-4 text-[13px] text-[#9a7fd1]">
            {requests
              .filter((r) => !r.available)
              .map((r) => `${r.label}: ${r.reason}`)
              .join(' ')}
          </div>
        )}
        {error && <div className="mt-2 bg-red-900 px-2 py-1 text-white">{error}</div>}
      </div>
      <BottomBar left="Cancelar" right="Confirmar" onLeft={() => onClose(null)} onRight={chosen ? () => void confirm() : undefined} />
    </div>
  )
}

export default function BoardPage() {
  const nav = useNav()
  useEscBack()
  const [view, setView] = useState<BoardConfidence | null>(null)
  const [meeting, setMeeting] = useState(false)
  const [answer, setAnswer] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = () => api.boardConfidence().then(setView, (e) => setError(String(e)))
  useEffect(() => {
    api.boardConfidence().then(setView, (e) => setError(String(e)))
  }, [])

  if (meeting) {
    return (
      <Meeting
        onClose={(a) => {
          setMeeting(false)
          if (a) {
            setAnswer(a)
            void load()
          }
        }}
      />
    )
  }

  const fg = view?.club.fg ?? '#000000'
  const bg = view?.club.bg ?? '#ffffff'
  return (
    <div className="flex h-full flex-col">
      <TitleBand title={view?.club.name ?? ''} fg={fg} bg={bg} />
      <div className="py-2 text-center font-title text-[30px] font-bold text-cm-yellow">Confiança da Direcção</div>
      <div className="flex shrink-0 gap-1 px-3">
        <button className="cm-purple h-6 w-40" onClick={() => setMeeting(true)}>
          Pedido à Direcção
        </button>
        <button className="cm-purple h-6 w-40" disabled title="Chega no próximo passo">
          Pedido à Liga
        </button>
      </div>
      <div className="mx-1.5 mt-1.5 min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/80 px-4 py-3">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        {answer && <p className="mb-5 font-title text-[20px] leading-snug text-cm-yellow">{answer}</p>}
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
