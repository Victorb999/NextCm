import { useEffect, useState } from 'react'
import { api } from '../api'
import type { JobsView } from '../bindings/JobsView'
import { BottomBar, Link, PurpleMenu, TitleBand } from '../components/Screen'
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
      <TitleBand title="Informação de Empregos" fg="#ffffff" bg="#1a1a1a" />
      <div className="py-2 text-center font-title text-[30px] font-bold text-cm-yellow">{view?.nation?.name ?? ''}</div>
      <div className="flex shrink-0 items-center gap-1 px-3">
        <PurpleMenu
          label="Vêr"
          options={(view?.nations ?? []).map((n) => ({ key: String(n.id), label: n.name }))}
          value={view?.nation ? String(view.nation.id) : undefined}
          onChange={(k) => setNation(Number(k))}
        />
        {view?.unemployed && <span className="ml-3 text-cm-magenta">Está sem clube: concorra a um lugar vago.</span>}
      </div>
      <div className="mx-1.5 mt-1.5 min-h-0 flex-1 overflow-auto border border-cm-line bg-cm-panel/80 px-4 py-2">
        {error && <div className="bg-red-900 px-2 py-1 text-white">{error}</div>}
        {notice && <div className="mb-1 text-cm-yellow">{notice}</div>}
        {view && view.rows.length === 0 && <div className="py-6 text-center text-cm-magenta">Não há lugares vagos nem treinadores em risco neste país.</div>}
        {view?.rows.map((r) => (
          <div key={r.club.id} className="flex h-[26px] items-center gap-3 font-title text-[19px]">
            <span className={`w-64 truncate ${r.vacant ? 'text-[#7b3fd1]' : 'text-white'}`}>{r.coach ?? 'Sem Treinador'}</span>
            <span className="w-56 truncate text-cm-magenta">
              <Link onClick={() => nav.go({ kind: 'club', id: r.club.id })}>{r.club.shortName}</Link>
            </span>
            <span className="w-32 truncate text-cm-yellow">{r.nation}</span>
            <span className="w-8 text-center text-cm-orange">{r.letter}</span>
            <span className="w-52 bg-black/40 px-2 text-center text-[15px] text-white">{r.situation}</span>
            {r.vacant && (
              <button className="cm-purple h-6 w-40 text-[13px]" disabled={r.applied} onClick={() => void apply(r.club.id)}>
                {r.applied ? 'Candidatura feita' : 'Concorrer ao Lugar'}
              </button>
            )}
          </div>
        ))}
      </div>
      <BottomBar onLeft={nav.back} />
    </div>
  )
}
