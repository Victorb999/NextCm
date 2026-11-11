import { Megaphone } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { TalkReaction } from '../bindings/TalkReaction'
import type { TalkView } from '../bindings/TalkView'
import { shortListName } from '../format'
import { BottomBar, ErrorBox, Panel, ScreenHeader } from './Screen'

// Palestra (antes do jogo, ao intervalo, no fim e ao suplente que entra), como as Instruções
// do FM08: a tabela do plantel (Nº, Jogador, Cond, Moral, Nota) com o que dizer a cada um;
// quem segue a equipa ouve as Instruções Gerais, "Pedir ao Adjunto" põe todos na frase que
// ele sugere, os suplentes por usar ficam apagados. Dar Palestra envia tudo de uma vez e
// mostra a reação de cada um. As regras estão no core.

const COLOURS: Record<string, string> = {
  Motivado: 'text-cm-green',
  Focado: 'text-[#a8e6a1]',
  'Sem reação': 'text-white',
  Complacente: 'text-cm-orange',
  Nervoso: 'text-cm-orange',
  Irritado: 'text-cm-red',
}

function Choice({ selected, onClick, children }: { selected: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button aria-pressed={selected} onClick={onClick} className="cm-option min-w-[90px] px-2 py-0.5">
      {children}
    </button>
  )
}

/** O que cada jogador ouve: 'team' = as Instruções Gerais, '' = nada, senão uma frase só dele. */
type Said = Record<number, string>

export default function TalkPanel({
  onClose,
  bottom = true,
}: {
  onClose: () => void
  /** A barra Atrás em baixo (no Pré-Jogo a barra é a dele). */
  bottom?: boolean
}) {
  const [view, setView] = useState<TalkView | null>(null)
  const [tone, setTone] = useState('assertive')
  // As Instruções Gerais (a frase da equipa) e o que cada um ouve
  const [general, setGeneral] = useState('')
  const [said, setSaid] = useState<Said>({})
  const [reactions, setReactions] = useState<TalkReaction[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const load = () =>
    api.talkView().then(
      (v) => {
        setView(v)
        // Antes de falar à equipa, todos seguem a sugestão do adjunto; depois, ninguém
        setGeneral(v.suggested)
        setSaid(Object.fromEntries(v.players.filter((p) => p.listens).map((p) => [p.id, v.done ? '' : 'team'])))
      },
      (e) => setError(String(e)),
    )
  useEffect(() => {
    void load()
  }, [])
  const lines = Object.entries(said)
    .map(([person, key]) => ({ person: Number(person), key: key === 'team' ? general : key }))
    .filter((l) => l.key)
  const talk = async () => {
    try {
      setReactions(await api.giveTalks(tone, lines))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }
  const again = () => {
    setReactions(null)
    void load()
  }
  // "Pedir ao Adjunto": todos voltam às Instruções Gerais, com a frase que ele sugere
  const askAssistant = () => {
    if (!view) return
    setGeneral(view.suggested)
    setSaid((m) => Object.fromEntries(Object.keys(m).map((k) => [k, 'team'])))
  }
  const phrase = (key: string) => view?.options.find((o) => o.key === key)?.label ?? ''

  const th = 'bg-cm-headdeep px-2 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow'
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Megaphone size={22} />} title={view?.title ?? 'Palestra'} />
      <div className="mx-2 flex min-h-0 flex-1 flex-col gap-2">
        <ErrorBox>{error}</ErrorBox>
        {view && !reactions && (
          <>
            <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 rounded-md bg-cm-surface px-4 py-2">
              <span className="text-[14px] font-semibold text-white">Tom</span>
              {view.tones.map((t) => (
                <Choice key={t.key} selected={tone === t.key} onClick={() => setTone(t.key)}>
                  {t.label}
                </Choice>
              ))}
              {view.team && view.done && (
                <span className="text-[13px] text-cm-magenta">Já falou à equipa: agora só a quem ainda não ouviu.</span>
              )}
            </div>
            <Panel
              title="Instruções"
              right={
                <span className="flex items-center gap-1.5">
                  <button className="cm-purple h-6 px-3" onClick={askAssistant}>
                    Pedir ao Adjunto
                  </button>
                  {/* Instruções Gerais: a frase de quem segue a equipa */}
                  <select
                    value={general}
                    onChange={(e) => setGeneral(e.target.value)}
                    className="cm-field h-6 w-60 rounded"
                    aria-label="Instruções Gerais"
                  >
                    {view.options.map((o) => (
                      <option key={o.key} value={o.key}>
                        Gerais: {o.label}
                      </option>
                    ))}
                  </select>
                  <button className="cm-purple h-6 px-4" disabled={lines.length === 0} onClick={() => void talk()}>
                    Dar Palestra
                  </button>
                </span>
              }
              className="min-h-0 flex-1"
            >
              <div className="min-h-0 flex-1 overflow-auto">
                <table className="w-full table-fixed border-collapse text-[14px]">
                  <thead className="sticky top-0 z-10">
                    <tr className="h-7">
                      <th className={`${th} w-12 text-center`}>Nº</th>
                      <th className={`${th} text-left`}>Jogador</th>
                      <th className={`${th} w-20 text-center`}>Cond</th>
                      <th className={`${th} w-28 text-center`}>Moral</th>
                      <th className={`${th} w-16 text-center`}>Nota</th>
                      <th className={`${th} w-80 text-center`}>Instruções</th>
                    </tr>
                  </thead>
                  <tbody>
                    {view.players.map((p) => (
                      <tr key={p.id} className={`h-7 ${p.listens ? 'text-white' : 'text-[#6c6c78]'}`}>
                        <td className="px-2 text-center">
                          <span
                            className={`inline-block w-8 rounded text-[12px] font-bold ${p.listens ? 'bg-cm-dlgline text-white' : 'bg-white/5'}`}
                          >
                            {p.number ?? ''}
                          </span>
                        </td>
                        <td className="truncate px-2 font-semibold">{shortListName(p.name)}</td>
                        <td className="px-2 text-center tabular-nums">{p.condition}%</td>
                        <td className="px-2 text-center">{p.morale}</td>
                        <td className="px-2 text-center tabular-nums">{p.rating ?? '-'}</td>
                        <td className="px-2 py-0.5">
                          {p.listens && (
                            <select
                              value={said[p.id] ?? ''}
                              onChange={(e) => setSaid((m) => ({ ...m, [p.id]: e.target.value }))}
                              className="cm-field h-6 w-full rounded"
                              aria-label={`Instruções para ${p.name}`}
                            >
                              <option value="team">Equipa ({phrase(general)})</option>
                              {view.options.map((o) => (
                                <option key={o.key} value={o.key}>
                                  {o.label}
                                </option>
                              ))}
                              <option value="">— Nada —</option>
                            </select>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </Panel>
          </>
        )}
        {reactions && (
          <Panel
            title="Reacções"
            right={
              <button className="cm-purple h-6 px-3" onClick={again}>
                Falar a outro jogador
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
      {/* Depois da palestra, o Seguinte sai da sala e volta à partida */}
      {bottom && <BottomBar left="Atrás" onLeft={onClose} right="Sair da Sala" onRight={reactions ? onClose : undefined} />}
    </div>
  )
}
