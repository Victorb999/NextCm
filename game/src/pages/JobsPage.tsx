import { Briefcase } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { JobsView } from '../bindings/JobsView'
import { BottomBar, Empty, ErrorBox, Link, Panel, PurpleMenu, ScreenHeader } from '../components/Screen'
import { useEscBack, useNav } from '../nav'

// "Informação de Empregos" (menu do treinador), como no CM: por país, os lugares vagos ("Sem
// Treinador", a roxo) e os treinadores inseguros. Num lugar vago, Concorrer; a resposta do
// clube chega às mensagens. As regras ficam no core.

export default function JobsPage() {
  const nav = useNav()
  useEscBack()
  const [view, setView] = useState<JobsView | null>(null)
  const [nation, setNation] = useState<number | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.jobsView(nation).then(
      (v) => {
        setView(v)
        setError(null)
      },
      (e) => setError(String(e)),
    )
  }, [nation])

  const apply = async (club: number) => {
    try {
      setNotice(await api.applyJob(club))
      setView(await api.jobsView(nation))
      setError(null)
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        icon={<Briefcase size={22} />}
        eyebrow={view?.nation?.name}
        title="Informação de Empregos"
        right={
          <PurpleMenu
            label="Vêr"
            align="right"
            options={(view?.nations ?? []).map((n) => ({ key: String(n.id), label: n.name }))}
            value={view?.nation ? String(view.nation.id) : undefined}
            onChange={(k) => setNation(Number(k))}
          />
        }
      />
      <div className="mx-2 flex min-h-0 flex-1 flex-col gap-2">
        <ErrorBox>{error}</ErrorBox>
        {view?.unemployed && <div className="shrink-0 rounded-md bg-[#3a1a4a] px-3 py-1.5 text-cm-magenta">Está sem clube: concorra a um lugar vago.</div>}
        {notice && <div className="shrink-0 rounded-md bg-[#0f3a2a] px-3 py-1.5 text-cm-yellow">{notice}</div>}
        <Panel title="Lugares vagos e treinadores em risco" className="mb-1 min-h-0 flex-1">
          <div className="flex h-7 shrink-0 items-center gap-3 border-b border-cm-line bg-black/30 px-3 font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-[#86869a]">
            <span className="w-56">Treinador</span>
            <span className="w-48">Clube</span>
            <span className="w-28">País</span>
            <span className="w-8 text-center">Div</span>
            <span className="flex-1">Situação</span>
          </div>
          <div className="cm-zebra min-h-0 flex-1 overflow-auto">
            {view && view.rows.length === 0 && <Empty className="py-6">Não há lugares vagos nem treinadores em risco neste país.</Empty>}
            {view?.rows.map((r) => (
              <div key={r.club.id} className="flex h-7 items-center gap-3 px-3 text-[14px]">
                <span className={`w-56 truncate font-semibold ${r.vacant ? 'text-[#9b6fe8]' : 'text-white'}`}>{r.coach ?? 'Sem Treinador'}</span>
                <span className="w-48 truncate text-cm-magenta">
                  <Link onClick={() => nav.go({ kind: 'club', id: r.club.id })}>{r.club.shortName}</Link>
                </span>
                <span className="w-28 truncate text-cm-yellow">{r.nation}</span>
                <span className="w-8 text-center font-mono text-cm-orange">{r.letter}</span>
                <span className="min-w-0 flex-1 truncate text-[#c8c8d0]">{r.situation}</span>
                {r.vacant && (
                  <button className="cm-purple h-6 w-40 shrink-0 text-[13px]" disabled={r.applied} onClick={() => void apply(r.club.id)}>
                    {r.applied ? 'Candidatura feita' : 'Concorrer ao Lugar'}
                  </button>
                )}
              </div>
            ))}
          </div>
        </Panel>
      </div>
      <BottomBar onLeft={nav.back} />
    </div>
  )
}
