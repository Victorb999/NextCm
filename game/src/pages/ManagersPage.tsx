import { Users } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { ManagersTable } from '../bindings/ManagersTable'
import { BottomBar, Empty, Link, PurpleMenu, ScreenHeader, type MenuOption } from '../components/Screen'
import { useEscBack, useNav } from '../nav'

// "Estatísticas dos Técnicos" (o Estatuto Treinador das Opções Jogo), como no CM: a reputação
// num país, a reputação mundial e a pontuação, 16 por página, com o treinador a amarelo.

const VIEWS: MenuOption<string>[] = [
  { key: 'nation', label: 'Reputação País' },
  { key: 'world', label: 'Reputação Mundial' },
  { key: 'score', label: 'Pontuação do Técnico' },
]

export default function ManagersPage() {
  const nav = useNav()
  useEscBack()
  const [view, setView] = useState('nation')
  const [nation, setNation] = useState<number | null>(null)
  // `null`: a página do treinador
  const [page, setPage] = useState<number | null>(null)
  const [data, setData] = useState<ManagersTable | null>(null)
  useEffect(() => {
    api.managersTable(view, nation, page).then(setData, () => setData(null))
  }, [view, nation, page])
  const at = data?.page ?? 0
  const pages = data?.pages ?? 1
  const nations: MenuOption<string>[] = (data?.nations ?? []).map((n) => ({ key: String(n.id), label: n.name }))

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader icon={<Users size={22} />} eyebrow="Estatísticas dos Técnicos" title={data?.title ?? 'Estatísticas dos Técnicos'} />
      <div className="flex shrink-0 items-center justify-between gap-3 px-3 pb-1">
        <div className="flex gap-2">
          <PurpleMenu
            label="Ver"
            options={VIEWS}
            value={view}
            onChange={(v) => {
              setView(v)
              setPage(null)
            }}
          />
          <PurpleMenu
            label="País"
            options={nations}
            value={String(data?.nationId ?? '')}
            onChange={(n) => {
              setNation(Number(n))
              setView('nation')
              setPage(null)
            }}
          />
        </div>
        <span className="flex items-center gap-1 text-[13px] text-white">
          <button className="cm-purple h-5 w-7" disabled={at === 0} onClick={() => setPage(0)}>
            «
          </button>
          <button className="cm-purple h-5 w-6" disabled={at === 0} onClick={() => setPage(at - 1)}>
            ‹
          </button>
          <span className="w-20 text-center text-cm-yellow">
            Pg {at + 1}/{pages}
          </span>
          <button className="cm-purple h-5 w-6" disabled={at + 1 >= pages} onClick={() => setPage(at + 1)}>
            ›
          </button>
          <button className="cm-purple h-5 w-7" disabled={at + 1 >= pages} onClick={() => setPage(pages - 1)}>
            »
          </button>
        </span>
      </div>
      <div className="mx-2 mb-1 min-h-0 flex-1 overflow-auto rounded-md bg-cm-surface">
        <table className="w-full table-fixed text-[15px]">
          <colgroup>
            <col className="w-14" />
            <col />
            <col className="w-[32%]" />
            <col className="w-32" />
          </colgroup>
          <tbody>
            {data?.rows.map((r) => (
              <tr key={r.personId} className="h-7">
                <td className="px-2 text-center font-mono text-[13px] text-white">{r.rank}º</td>
                <td className="truncate px-2">
                  <Link className={r.me ? '!text-cm-yellow' : ''} onClick={() => nav.go({ kind: 'person', id: r.personId })}>
                    {r.name}
                  </Link>
                </td>
                <td className="truncate px-2 text-[13px] font-bold text-cm-yellow">
                  {r.team ? <Link onClick={() => nav.go({ kind: 'club', id: r.team!.id })}>{r.team.shortName}</Link> : 'Sem clube'}
                </td>
                <td className="px-2 text-center text-[13px] text-white">
                  {data.view === 'score' ? r.score.toLocaleString('pt-BR') : r.level}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {data && data.rows.length === 0 && <Empty>Sem técnicos neste país.</Empty>}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
