import { Trophy } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import type { HonourAward } from '../bindings/HonourAward'
import type { HonoursBoard } from '../bindings/HonoursBoard'
import type { ClubRef } from '../bindings/ClubRef'
import { BottomBar, Empty, Link, PurpleMenu, ScreenHeader, type MenuOption } from '../components/Screen'
import { useEscBack, useNav } from '../nav'

// "Quadro de Honra" das Opções Jogo: os campeões e os vices de cada competição viva num ano,
// por país ou as internacionais de clubes, com o Melhor Jogador, o Artilheiro e a Bola de Ouro.

export default function HonoursPage() {
  const nav = useNav()
  useEscBack()
  const [group, setGroup] = useState<number | null>(null)
  const [year, setYear] = useState<number | null>(null)
  const [data, setData] = useState<HonoursBoard | null>(null)
  useEffect(() => {
    api.honoursBoard(group, year).then(setData, () => setData(null))
  }, [group, year])
  const years = data?.years ?? []
  const at = data?.year != null ? years.indexOf(data.year) : -1
  const groups: MenuOption<string>[] = (data?.groups ?? []).map((g) => ({ key: String(g.id), label: g.name }))
  const groupName = data?.groups.find((g) => g.id === data.group)?.name

  const club = (c: ClubRef | null) =>
    c ? <Link onClick={() => nav.go({ kind: 'club', id: c.id })}>{c.shortName}</Link> : <span className="text-white/40">—</span>
  const award = (a: HonourAward | null, unit?: string) =>
    a ? (
      <>
        <Link onClick={() => nav.go({ kind: 'person', id: a.personId })}>{a.name}</Link>
        {a.value != null && unit && (
          <span className="ml-1 text-[12px] text-white/60">
            ({Math.round(a.value)} {unit})
          </span>
        )}
      </>
    ) : (
      <span className="text-white/40">—</span>
    )

  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        icon={<Trophy size={22} />}
        eyebrow={groupName}
        title="Quadro de Honra"
        right={
          <>
            {/* Os anos vão do mais recente para o mais antigo: ◀ é o ano antes */}
            <button className="cm-purple h-7 w-9 px-0" disabled={at < 0 || at + 1 >= years.length} onClick={() => setYear(years[at + 1]!)}>
              ◀
            </button>
            <span className="w-20 text-center font-title text-[17px] font-bold text-cm-yellow">{data?.year ?? '—'}</span>
            <button className="cm-purple h-7 w-9 px-0" disabled={at <= 0} onClick={() => setYear(years[at - 1]!)}>
              ▶
            </button>
          </>
        }
      />
      <div className="flex shrink-0 items-center gap-2 px-3 pb-1">
        <PurpleMenu
          label="País"
          options={groups}
          value={String(data?.group ?? '')}
          onChange={(g) => {
            setGroup(Number(g))
            setYear(null)
          }}
        />
        <PurpleMenu
          label="Ano"
          options={years.map((y) => ({ key: String(y), label: String(y) }))}
          value={String(data?.year ?? '')}
          onChange={(y) => setYear(Number(y))}
        />
      </div>
      <div className="mx-2 mb-1 min-h-0 flex-1 overflow-auto rounded-md bg-cm-surface">
        <table className="w-full table-fixed text-[14px]">
          <colgroup>
            <col className="w-[22%]" />
            <col className="w-20" />
            <col />
            <col />
            <col className="w-[19%]" />
            <col className="w-[19%]" />
          </colgroup>
          <thead className="sticky top-0">
            <tr className="h-7">
              {['Competição', 'Temporada', 'Campeão', 'Vice', 'Melhor Jogador', 'Artilheiro'].map((h) => (
                <th
                  key={h}
                  className="bg-cm-headdeep px-2 text-left font-mono text-[11px] font-bold uppercase tracking-[0.1em] text-cm-yellow"
                >
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data?.ballon && (
              <tr className="h-6">
                <td className="truncate px-2 text-white">Bola de Ouro</td>
                <td className="px-2 text-white">{data.year}</td>
                <td className="truncate px-2" colSpan={2}>
                  {award(data.ballon)}
                </td>
                <td className="truncate px-2 text-cm-yellow">{data.ballon.club?.shortName}</td>
                <td />
              </tr>
            )}
            {data?.rows.map((r) => (
              <tr key={r.competitionId} className="h-6">
                <td className="truncate px-2">
                  <Link onClick={() => nav.go({ kind: 'competition', id: r.competitionId })}>{r.competition}</Link>
                </td>
                <td className="px-2 text-white">{r.season}</td>
                <td className="truncate px-2 font-bold text-cm-yellow">{club(r.winner)}</td>
                <td className="truncate px-2 text-cm-magenta">{club(r.runnerUp)}</td>
                <td className="truncate px-2">{award(r.bestPlayer)}</td>
                <td className="truncate px-2">{award(r.topScorer, 'gols')}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {data && data.rows.length === 0 && !data.ballon && <Empty>Sem campeões neste ano.</Empty>}
      </div>
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
