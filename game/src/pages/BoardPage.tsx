import { Euro, Landmark, Star, Users, Zap } from 'lucide-react'
import { Fragment, useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { BoardConfidence } from '../bindings/BoardConfidence'
import type { BoardLine } from '../bindings/BoardLine'
import type { BoardRequest } from '../bindings/BoardRequest'
import { BottomBar, ErrorBox, Link, Panel, ScreenHeader } from '../components/Screen'
import { money, wage } from '../format'
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
      <Panel title={meeting.prompt} className="mx-2 min-h-0 flex-1">
        <div className="cm-zebra min-h-0 flex-1 overflow-auto">
          {requests.map((r) => (
            <button
              key={r.key}
              disabled={!r.available}
              title={r.reason ?? undefined}
              onClick={() => setChosen(r.key)}
              className={`flex h-[34px] w-full items-center gap-3 px-4 text-left text-[16px] ${
                !r.available
                  ? 'cursor-default text-[#7b3fd1]'
                  : chosen === r.key
                    ? '!bg-cm-sel font-bold text-white'
                    : 'text-white hover:text-cm-yellow'
              }`}
            >
              <span className="min-w-0 flex-1 truncate">{r.label}</span>
              {!r.available && r.reason && <span className="max-w-[55%] truncate text-[12px] text-[#9a7fd1]">{r.reason}</span>}
            </button>
          ))}
        </div>
        {reason && <div className="border-t border-cm-line px-4 py-2 text-cm-magenta">{reason}</div>}
        <ErrorBox className="m-2">{error}</ErrorBox>
      </Panel>
      <BottomBar left="Cancelar" right="Confirmar" onLeft={() => onClose(null)} onRight={chosen ? () => void confirm() : undefined} />
    </div>
  )
}

const LINE_STYLE: Record<string, { icon: ReactNode; colour: string; bar: string }> = {
  performance: { icon: <Star size={20} />, colour: 'text-[#4edea3]', bar: 'bg-[#10b981]' },
  finances: { icon: <Euro size={20} />, colour: 'text-cm-orange', bar: 'bg-cm-orange' },
  fans: { icon: <Users size={20} />, colour: 'text-[#5bc0f0]', bar: 'bg-[#1e9cf0]' },
  players: { icon: <Zap size={20} />, colour: 'text-[#b48cf0]', bar: 'bg-[#7c3aed]' },
}

/** O texto da frase com os nomes dos jogadores como ligações. */
function LineText({ line }: { line: BoardLine }) {
  const nav = useNav()
  if (!line.players.length) return <>{line.text}</>
  const parts: ReactNode[] = []
  let rest = line.text
  for (const [id, name] of line.players) {
    const at = rest.indexOf(name)
    if (at < 0) continue
    parts.push(rest.slice(0, at))
    parts.push(
      <Link key={id} className="font-bold text-cm-yellow" onClick={() => nav.go({ kind: 'person', id })}>
        {name}
      </Link>,
    )
    rest = rest.slice(at + name.length)
  }
  parts.push(rest)
  return (
    <>
      {parts.map((p, i) => (
        <Fragment key={i}>{p}</Fragment>
      ))}
    </>
  )
}

export default function BoardPage() {
  const nav = useNav()
  useEscBack()
  const [view, setView] = useState<BoardConfidence | null>(null)
  const [meeting, setMeeting] = useState<Kind | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.boardConfidence().then(setView, (e) => setError(String(e)))
  }, [])

  if (meeting) {
    return (
      <Meeting
        kind={meeting}
        onClose={(a) => {
          setMeeting(null)
          // A resposta chega como mensagem: volta às Notícias, como no CM
          if (a) nav.go({ kind: 'news' })
        }}
      />
    )
  }

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        club={view?.club}
        eyebrow={<span className="text-cm-orange">{view ? `${view.club.name}${view.division ? ` · ${view.division}` : ''}` : ''}</span>}
        title="Confiança da Direcção"
        right={
          <>
            <button className="cm-purple h-8 px-4" onClick={() => setMeeting('board')}>
              Pedido à Direcção
            </button>
            <button className="cm-purple h-8 px-4" onClick={() => setMeeting('league')}>
              Pedido à Liga
            </button>
          </>
        }
      />
      <div className="mx-2 flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
        <ErrorBox>{error}</ErrorBox>
        <div className="shrink-0 overflow-hidden rounded-md bg-[#1a1b21]">
          <div className="cm-zebra">
            {view?.lines.map((l) => {
              const st = LINE_STYLE[l.kind] ?? LINE_STYLE.performance
              return (
                <div key={l.kind} className="flex items-center gap-4 px-4 py-3">
                  <span className={`flex size-10 shrink-0 items-center justify-center rounded bg-black/40 ${st.colour}`}>{st.icon}</span>
                  <div className="min-w-0 flex-1">
                    <div className={`font-mono text-[11px] font-bold uppercase tracking-[0.12em] ${st.colour}`}>{l.label}</div>
                    <div className="text-[17px] leading-snug text-white">
                      <LineText line={l} />
                    </div>
                    {l.level !== null && (
                      <div className="mt-1.5 h-1.5 overflow-hidden rounded bg-black/50">
                        <div className={`h-full rounded ${st.bar}`} style={{ width: `${l.level}%` }} />
                      </div>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        </div>
        {view && (
          <div className="flex shrink-0 flex-wrap gap-x-6 gap-y-1 rounded-md bg-[#1a1b21] px-4 py-2 font-mono text-[12px] uppercase tracking-wider text-[#86869a]">
            <span>
              Expectativa: <b className="normal-case text-white">{view.goal.charAt(0).toUpperCase() + view.goal.slice(1)}</b>
            </span>
            <span>
              Orçamento: <b className="text-[#4edea3]">{money(view.transferBudget) || '€0'}</b>
            </span>
            <span>
              Massa salarial: <b className="normal-case text-white">{wage(view.wageBill)}</b> / {wage(view.wageBudget)}
            </span>
          </div>
        )}
      </div>
      <BottomBar onLeft={nav.back} />
    </div>
  )
}
