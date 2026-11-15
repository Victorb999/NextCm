import { Trophy } from 'lucide-react'
import { useState } from 'react'
import { api } from '../api'
import type { MatchSheet } from '../bindings/MatchSheet'
import { MatchHistoryBody } from '../components/match/MatchHistory'
import { BottomBar, Empty, ScreenHeader } from '../components/Screen'
import { longDate } from '../format'
import { useEscBack, useNav } from '../nav'
import { useLoad } from '../useLoad'

// A partida jogada (ao clicar num resultado nas notícias, no calendário, nas listas): a tela do
// CM com a Visão Geral, as estatísticas, as zonas, o relatório, os Dados de cada clube e as
// Classificações Jogadores (`MatchHistory`).

export default function MatchSheetPage({ id }: { id: number }) {
  const nav = useNav()
  useEscBack()
  const [sheet, setSheet] = useState<MatchSheet | null>(null)
  const [error, setError] = useState<string | null>(null)
  useLoad(() => api.matchSheet(id), [id], setSheet, setError)
  const f = sheet?.fixture
  return (
    <div className="flex h-full flex-col">
      <ScreenHeader
        icon={<Trophy size={22} />}
        eyebrow={f && <span className="text-cm-orange">{f.competition}</span>}
        title={f ? `${f.label} · ${longDate(f.date)}` : 'Partida'}
      />
      {error && <Empty>{error}</Empty>}
      {sheet && (
        <div className="mx-2 flex min-h-0 flex-1 flex-col">
          <MatchHistoryBody sheet={sheet} />
        </div>
      )}
      <BottomBar onLeft={nav.canBack ? nav.back : undefined} onRight={nav.canForward ? nav.forward : undefined} />
    </div>
  )
}
