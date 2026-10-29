import { Megaphone } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TalkReaction } from '../bindings/TalkReaction'
import type { TalkView } from '../bindings/TalkView'
import { BottomBar, ErrorBox, ScreenHeader } from './Screen'

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
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col gap-2 border border-cm-line bg-cm-panel/70 px-6 py-3">
        <ErrorBox>{error}</ErrorBox>
        {view && !reactions && (
          <>
            <div className="flex items-center gap-2">
              <span className="w-24 font-title text-[18px] text-white">A quem</span>
              <select
                value={person ?? ''}
                onChange={(e) => setPerson(e.target.value ? Number(e.target.value) : null)}
                className="cm-field min-w-64"
              >
                {view.team && !teamDone && <option value="">À equipa toda</option>}
                {view.players.map((p) => (
                  <option key={p.key} value={p.key}>
                    {p.label}
                  </option>
                ))}
              </select>
              {view.team && teamDone && <span className="text-[13px] text-cm-magenta">Já falou à equipa: agora só a um jogador.</span>}
            </div>
            <div className="flex items-center gap-2">
              <span className="w-24 font-title text-[18px] text-white">Tom</span>
              {view.tones.map((t) => (
                <Choice key={t.key} selected={tone === t.key} onClick={() => setTone(t.key)}>
                  {t.label}
                </Choice>
              ))}
            </div>
            <div className="flex flex-col gap-1.5 pt-1">
              {view.options.map((o) => (
                <button
                  key={o.key}
                  disabled={person === null && teamDone}
                  className="cm-purple min-h-9 px-4 py-1 text-left font-title text-[18px]"
                  onClick={() => void talk(o.key)}
                >
                  {o.label}
                  {o.key === view.suggested && <span className="ml-3 text-[13px] text-cm-yellow">(sugestão do adjunto)</span>}
                </button>
              ))}
            </div>
          </>
        )}
        {reactions && (
          <>
            <div className="min-h-0 flex-1 overflow-auto">
              {reactions.map((r) => (
                <div key={r.personId} className="flex h-[22px] items-center justify-between border-b border-cm-line px-2 text-[15px]">
                  <span className="text-white">{r.name}</span>
                  <span className={`font-bold ${COLOURS[r.reaction] ?? 'text-white'}`}>{r.reaction}</span>
                </div>
              ))}
            </div>
            <div className="flex justify-end">
              <button className="cm-purple h-8 w-56" onClick={again}>
                Falar a um jogador
              </button>
            </div>
          </>
        )}
      </div>
      <BottomBar left="Atrás" onLeft={onClose} />
    </div>
  )
}
