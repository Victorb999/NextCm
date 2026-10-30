import { Megaphone } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TalkReaction } from '../bindings/TalkReaction'
import type { TalkView } from '../bindings/TalkView'
import { BottomBar, ErrorBox, Panel, ScreenHeader } from './Screen'

// Palestra (antes do jogo, ao intervalo, no fim e ao suplente que entra): o tom, a quem
// (a equipa ou um jogador), a frase (com a sugestão do adjunto) e a reação de cada um.
// As regras estão no core.

const COLOURS: Record<string, string> = {
  Motivado: 'text-[#4cd964]',
  Focado: 'text-[#a8e6a1]',
  'Sem reação': 'text-white',
  Complacente: 'text-cm-orange',
  Nervoso: 'text-cm-orange',
  Irritado: 'text-[#ff5050]',
}

function Choice({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button aria-pressed={selected} onClick={onClick} className="cm-option min-w-[90px] px-2 py-0.5">
      {children}
    </button>
  )
}

export default function TalkPanel({ onClose }: { onClose: () => void }) {
  const [view, setView] = useState<TalkView | null>(null)
  const [tone, setTone] = useState('assertive')
  // null = a equipa toda
  const [person, setPerson] = useState<number | null>(null)
  const [reactions, setReactions] = useState<TalkReaction[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const load = () =>
    api.talkView().then(
      (v) => {
        setView(v)
        // Ao suplente fala-se a um de cada vez; à equipa depois de já ter falado, também
        setPerson(!v.team || v.done ? Number(v.players[0]?.key ?? 0) || null : null)
      },
      (e) => setError(String(e)),
    )
  useEffect(() => {
    void load()
  }, [])
  const talk = async (key: string) => {
    try {
      setReactions(await api.giveTalk(key, tone, person))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }
  const again = () => {
    setReactions(null)
    void load()
  }
  const teamDone = !!view?.done
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Megaphone size={22} />} title={view?.title ?? 'Palestra'} />
      <div className="mx-2 flex min-h-0 flex-1 flex-col gap-2">
        <ErrorBox>{error}</ErrorBox>
        {view && !reactions && (
          <>
            <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 rounded-md bg-[#1a1b21] px-4 py-2">
              <span className="text-[14px] font-semibold text-white">A quem</span>
              <select
                value={person ?? ''}
                onChange={(e) => setPerson(e.target.value ? Number(e.target.value) : null)}
                className="cm-field h-7 min-w-64 rounded"
              >
                {view.team && !teamDone && <option value="">À equipa toda</option>}
                {view.players.map((p) => (
                  <option key={p.key} value={p.key}>
                    {p.label}
                  </option>
                ))}
              </select>
              <span className="ml-4 text-[14px] font-semibold text-white">Tom</span>
              {view.tones.map((t) => (
                <Choice key={t.key} selected={tone === t.key} onClick={() => setTone(t.key)}>
                  {t.label}
                </Choice>
              ))}
              {view.team && teamDone && <span className="text-[13px] text-cm-magenta">Já falou à equipa: agora só a um jogador.</span>}
            </div>
            <Panel title="O que dizer" className="shrink-0">
              <div className="cm-zebra">
                {view.options.map((o) => (
                  <button
                    key={o.key}
                    disabled={person === null && teamDone}
                    className="flex min-h-9 w-full items-center gap-3 px-4 py-1 text-left text-[15px] text-white hover:!bg-cm-sel disabled:text-[#6c6c78]"
                    onClick={() => void talk(o.key)}
                  >
                    <span className="flex-1">{o.label}</span>
                    {o.key === view.suggested && <span className="shrink-0 font-mono text-[11px] uppercase tracking-wider text-cm-yellow">Sugestão do adjunto</span>}
                  </button>
                ))}
              </div>
            </Panel>
          </>
        )}
        {reactions && (
          <Panel
            title="Reacções"
            right={
              <button className="cm-purple h-6 px-3" onClick={again}>
                Falar a um jogador
              </button>
            }
            className="min-h-0 flex-1"
          >
            <div className="cm-zebra min-h-0 flex-1 overflow-auto">
              {reactions.map((r) => (
                <div key={r.personId} className="flex h-6 items-center justify-between px-4 text-[14px]">
                  <span className="text-white">{r.name}</span>
                  <span className={`font-bold ${COLOURS[r.reaction] ?? 'text-white'}`}>{r.reaction}</span>
                </div>
              ))}
            </div>
          </Panel>
        )}
      </div>
      <BottomBar left="Atrás" onLeft={onClose} />
    </div>
  )
}
