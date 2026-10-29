import { Landmark } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { BoardConfidence } from '../bindings/BoardConfidence'
import type { BoardRequest } from '../bindings/BoardRequest'
import { BottomBar, ErrorBox, ScreenHeader } from '../components/Screen'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// "Confiança da Direcção" (menu do treinador): o que a direção pensa do treinador, das finanças,
// dos adeptos e dos jogadores valiosos, em parágrafos como no CM. "Pedido à Direcção" abre a
// Reunião com a Direcção e "Pedido à Liga" a Reunião com a Federação (as regras e as
// respostas vêm do core).

type Kind = 'board' | 'league'
const MEETINGS: Record<Kind, { title: string; prompt: string; list: () => Promise<BoardRequest[]>; ask: (key: string) => Promise<string> }> = {
  board: {
    title: 'Reunião com a Direcção',
    prompt: 'Especificar os detalhes das exigências à direcção:',
    list: api.boardRequests,
    ask: api.boardRequest,
  },
  league: {
    title: 'Reunião com a Federação',
    prompt: 'Especifique as questões que deseja discutir com a Liga',
    list: api.leagueRequests,
    ask: api.leagueRequest,
  },
}

function Meeting({ kind, onClose }: { kind: Kind; onClose: (answer: string | null) => void }) {
  const meeting = MEETINGS[kind]
  const game = useGame()
  const [requests, setRequests] = useState<BoardRequest[]>([])
  const [chosen, setChosen] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    meeting.list().then(setRequests, (e) => setError(String(e)))
  }, [meeting])

  const confirm = async () => {
    if (!chosen) return
    try {
      const answer = await meeting.ask(chosen)
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
      <ScreenHeader icon={<Landmark size={22} />} eyebrow={game.info?.managerName} title={meeting.title} />
      <div className="mx-1.5 min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/80 px-6 py-4">
        <div className="pb-3 font-title text-[20px] text-white">{meeting.prompt}</div>
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
        <ErrorBox className="mt-2">{error}</ErrorBox>
      </div>
      <BottomBar left="Cancelar" right="Confirmar" onLeft={() => onClose(null)} onRight={chosen ? () => void confirm() : undefined} />
    </div>
  )
}

export default function BoardPage() {
  const nav = useNav()
  useEscBack()
  const [view, setView] = useState<BoardConfidence | null>(null)
  const [meeting, setMeeting] = useState<Kind | null>(null)
  const [answer, setAnswer] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = () => api.boardConfidence().then(setView, (e) => setError(String(e)))
  useEffect(() => {
    api.boardConfidence().then(setView, (e) => setError(String(e)))
  }, [])

  if (meeting) {
    return (
      <Meeting
        kind={meeting}
        onClose={(a) => {
          setMeeting(null)
          if (a) {
            setAnswer(a)
            void load()
          }
        }}
      />
    )
  }

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader club={view?.club} eyebrow={view?.club.name} title="Confiança da Direcção" />
      <div className="flex shrink-0 gap-1 px-3">
        <button className="cm-purple h-6 w-40" onClick={() => setMeeting('board')}>
          Pedido à Direcção
        </button>
        <button className="cm-purple h-6 w-40" onClick={() => setMeeting('league')}>
          Pedido à Liga
        </button>
      </div>
      <div className="mx-1.5 mt-1.5 min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/80 px-4 py-3">
        <ErrorBox>{error}</ErrorBox>
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
