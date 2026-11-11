import { useLoad } from '../useLoad'
import { Trophy } from 'lucide-react'
import { useState } from 'react'
import { api } from '../api'
import type { MatchSheet } from '../bindings/MatchSheet'
import MatchLineups from '../components/MatchLineups'
import { BottomBar, Empty, Link, Panel, ScreenHeader } from '../components/Screen'
import { longDate, pensText } from '../format'
import { sheetLineup } from '../matchLineup'
import { useEscBack, useNav } from '../nav'

// A ficha de uma partida jogada (ao clicar num resultado): as faixas dos dois clubes com o
// placar e as "Classificações Jogadores" do CM, com quem saiu e quem entrou.

export default function MatchSheetPage({ id }: { id: number }) {
  const nav = useNav()
  useEscBack()
  const [sheet, setSheet] = useState<MatchSheet | null>(null)
  const [error, setError] = useState<string | null>(null)
  useLoad(() => api.matchSheet(id), [id], setSheet, setError)

  const f = sheet?.fixture
  const band = (club: NonNullable<typeof f>['home'], goals: number | null, right: boolean) => (
    <div
      className={`flex h-14 flex-1 items-center gap-3 rounded-md px-4 ${right ? 'flex-row-reverse' : ''}`}
      style={{ color: club.fg ?? '#fff', background: club.bg ?? '#333' }}
    >
      <span className="min-w-0 flex-1 truncate font-title text-[26px] font-bold" style={{ textAlign: right ? 'right' : 'left' }}>
        <Link onClick={() => nav.go({ kind: 'club', id: club.id })}>{club.shortName}</Link>
      </span>
      <span className="flex h-10 w-12 items-center justify-center rounded bg-black/80 font-title text-[26px] font-bold text-white">
        {goals ?? ''}
      </span>
    </div>
  )
  // A melhor nota do jogo: o homem do jogo
  const motm = sheet ? ([...sheet.home, ...sheet.away].find((l) => l.mom)?.id ?? null) : null
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        icon={<Trophy size={22} />}
        eyebrow={f && <span className="text-cm-orange">{f.competition}</span>}
        title={f ? `${f.label} · ${longDate(f.date)}` : 'Partida'}
      />
      {error && <Empty>{error}</Empty>}
      {sheet && f && (
        <div className="mx-2 flex min-h-0 flex-1 flex-col gap-2">
          <div className="flex shrink-0 gap-2">
            {band(f.home, f.homeGoals, false)}
            {band(f.away, f.awayGoals, true)}
          </div>
          {pensText(f) && <div className="shrink-0 text-center text-[14px] text-cm-yellow">{pensText(f)}</div>}
          <Panel title="Classificações Jogadores" className="min-h-0 flex-1">
            <div className="min-h-0 flex-1 overflow-auto">
              <MatchLineups
                home={{ club: f.home, entries: sheetLineup(sheet.home) }}
                away={{ club: f.away, entries: sheetLineup(sheet.away) }}
                motm={motm}
              />
            </div>
          </Panel>
        </div>
      )}
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
