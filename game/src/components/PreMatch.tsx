import { Trophy } from 'lucide-react'
import { useEffect, useImperativeHandle, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { LiveTeam } from '../bindings/LiveTeam'
import type { LiveView } from '../bindings/LiveView'
import type { MatchAnalysis } from '../bindings/MatchAnalysis'
import type { MatchReview } from '../bindings/MatchReview'
import { shortListName } from '../format'
import PitchField from './PitchField'
import { BottomBar, Panel, ScreenHeader, Tabs } from './Screen'
import TalkPanel from './TalkPanel'

// O Pré-Jogo (marco 3.21), como o Preview e a Team Talk do FM08 no visual do CM: as Equipas nos
// dois campos, a Análise (odds, homem-chave, o jornal, os marcos e a antevisão) e a Palestra.
// Os números vêm todos do core; Ir para o Jogo passa à partida.

type Tab = 'teams' | 'analysis' | 'talk'
const TABS: { key: Tab; label: string }[] = [
  { key: 'teams', label: 'Equipas' },
  { key: 'analysis', label: 'Análise' },
  { key: 'talk', label: 'Palestra' },
]

/** Um onze no seu campo de pé, com o número nas cores do clube e o apelido; os suplentes por baixo. */
function TeamSheet({ team }: { team: LiveTeam }) {
  const c = team.club
  const colours = c.fg && c.bg ? { color: c.fg, background: c.bg } : { color: '#141414', background: '#ffffff' }
  return (
    <Panel title={c.shortName} chip={team.formation} className="min-w-0 flex-1">
      <div className="flex min-h-0 flex-1 items-center justify-center p-2 [container-type:size]">
        <PitchField style={{ width: 'min(100cqw, 100cqh * 0.7)' }}>
          {team.players
            .filter((p) => p.x !== null && p.y !== null)
            .map((p) => (
              <div
                key={p.id}
                className="absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center gap-0.5"
                style={{ left: `${p.x}%`, top: `${p.y}%` }}
                title={p.role ?? ''}
              >
                <span
                  className="flex size-8 items-center justify-center rounded-lg text-[14px] font-bold shadow-[0_2px_6px_rgba(0,0,0,0.5)] ring-1 ring-black/40"
                  style={p.role === 'GR' ? { background: 'rgb(var(--c-mint))', color: 'rgb(var(--c-onmint))' } : colours}
                >
                  {p.squadNumber ?? ''}
                </span>
                <span className="whitespace-nowrap rounded bg-black/75 px-1.5 text-[12px] font-bold leading-[18px] text-white">
                  {shortListName(p.listName)}
                </span>
              </div>
            ))}
        </PitchField>
      </div>
      <div className="shrink-0 border-t border-cm-line px-3 py-1.5 text-[13px] leading-5">
        <span className="font-bold text-cm-orange">Suplentes: </span>
        <span className="text-cm-muted">
          {team.bench.map((p) => `${p.squadNumber ?? '-'} ${shortListName(p.listName)}`).join(' · ') || '-'}
        </span>
      </div>
    </Panel>
  )
}

/** Uma linha "rótulo ... valor" da Análise. */
function Row({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="flex min-h-[26px] items-baseline gap-4 px-3 py-0.5">
      <span className="w-56 shrink-0 text-[15px] font-semibold text-white">{label}</span>
      <span className="min-w-0 flex-1 text-[15px] text-cm-mint">{children}</span>
    </div>
  )
}

function Analysis({ view, a }: { view: LiveView; a: MatchAnalysis }) {
  const [home, away] = [view.home.club.shortName, view.away.club.shortName]
  const fav = (side: number) => (a.favourite === side ? ' (Fav.)' : '')
  const last = a.preview.lastMeeting
  const milestones = (side: 0 | 1, club: string) => (
    <Panel title={`Marcos · ${club}`} className="min-w-0 flex-1">
      <div className="cm-zebra py-1">
        {a.milestones[side].length === 0 ? (
          <div className="px-3 py-1 text-[14px] text-cm-dim">Não há marcos para o {club} neste jogo.</div>
        ) : (
          a.milestones[side].map((m) => (
            <div key={m.id} className="flex h-[26px] items-center gap-3 px-3 text-[14px]">
              <span className="w-52 shrink-0 truncate font-semibold text-white">{m.name}</span>
              <span className="truncate text-cm-mint">{m.text}</span>
            </div>
          ))
        )}
      </div>
    </Panel>
  )
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 px-2">
      <Panel title="Previsões">
        <div className="cm-zebra py-1">
          <Row label="Odds">
            {home}: {a.odds[0].toFixed(2)}
            {fav(0)} · Empate: {a.odds[1].toFixed(2)} · {away}: {a.odds[2].toFixed(2)}
            {fav(1)}
          </Row>
          <Row label={`Homem-Chave (${home})`}>
            {a.keyMen[0] ? `${a.keyMen[0].name}${a.keyMen[0].role ? ` (${a.keyMen[0].role})` : ''}` : '-'}
          </Row>
          <Row label={`Homem-Chave (${away})`}>
            {a.keyMen[1] ? `${a.keyMen[1].name}${a.keyMen[1].role ? ` (${a.keyMen[1].role})` : ''}` : '-'}
          </Row>
          <Row label={`Previsão do ${a.paper}`}>{a.prediction}</Row>
        </div>
      </Panel>
      <Panel title="Antevisão">
        <div className="cm-zebra py-1">
          <Row label="Estádio">{[a.preview.venue, a.preview.city].filter(Boolean).join(', ') || '-'}</Row>
          <Row label="Tempo">{a.preview.weather}</Row>
          <Row label="Condições Terreno Jogo">{a.preview.pitch}</Row>
          <Row label="Último Confronto">
            {last
              ? `${last.home.shortName} ${last.homeGoals ?? '-'}-${last.awayGoals ?? '-'} ${last.away.shortName} (${last.competition})`
              : 'Primeiro jogo entre os dois'}
          </Row>
          <Row label={`Ausências (${home})`}>{a.preview.homeNews.join(' · ') || 'Nenhuma'}</Row>
          <Row label={`Ausências (${away})`}>{a.preview.awayNews.join(' · ') || 'Nenhuma'}</Row>
        </div>
      </Panel>
      <div className="flex min-h-0 gap-2">
        {milestones(0, home)}
        {milestones(1, away)}
      </div>
    </div>
  )
}

export default function PreMatch({
  ref,
  onPlay,
}: {
  /** O Continuar Jogo da coluna (e o Enter), para o pai a chamar. */
  ref?: React.Ref<{ main: () => void }>
  onPlay: () => void
}) {
  const [tab, setTab] = useState<Tab>('teams')
  const [view, setView] = useState<LiveView | null>(null)
  const [analysis, setAnalysis] = useState<MatchAnalysis | null>(null)
  const [error, setError] = useState<string | null>(null)
  // A palestra já foi dada (ou a aba já foi vista): o Continuar vai então ao jogo
  const [talked, setTalked] = useState(false)
  useEffect(() => {
    if (tab === 'talk') setTalked(true)
  }, [tab])
  useEffect(() => {
    api.talkView().then(
      (t) => t.done && setTalked(true),
      () => {},
    )
  }, [])
  // Continuar: primeiro a Palestra (fica escondida e esquece-se), depois o jogo
  const play = () => (talked ? onPlay() : setTab('talk'))
  useImperativeHandle(ref, () => ({ main: play }))
  useEffect(() => {
    api.liveMatch().then(
      (v) => {
        // Uma partida já começada (voltar ao dia de jogo) vai direto ao jogo
        if (v.minute > 0) onPlay()
        else setView(v)
      },
      (e) => setError(String(e)),
    )
    api.matchAnalysis().then(setAnalysis, (e) => setError(String(e)))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  const at = TABS.findIndex((t) => t.key === tab)
  const last = at === TABS.length - 1
  return (
    <div className="cm-backdrop absolute inset-0 z-50 flex flex-col">
      <ScreenHeader
        icon={
          <span className="flex size-11 items-center justify-center rounded bg-[#1e9cf0] text-white">
            <Trophy size={22} />
          </span>
        }
        eyebrow={view ? `${view.fixture.competition} · ${view.fixture.label}` : 'Pré-Jogo'}
        title={view ? `Pré-Jogo · ${view.home.club.shortName} v ${view.away.club.shortName}` : 'Pré-Jogo'}
        right={
          <button className="cm-purple h-8 px-4 font-title text-[16px]" onClick={play}>
            {talked ? 'Ir para o Jogo' : 'Palestra'}
          </button>
        }
      />
      <Tabs tabs={TABS} active={tab} onChange={(k) => setTab(k as Tab)} />
      <div className="mt-2 flex min-h-0 flex-1 flex-col">
        {error && <div className="p-4 text-cm-red">{error}</div>}
        {view && tab === 'teams' && (
          <div className="flex min-h-0 flex-1 gap-2 px-2">
            <TeamSheet team={view.home} />
            <TeamSheet team={view.away} />
          </div>
        )}
        {view && analysis && tab === 'analysis' && <Analysis view={view} a={analysis} />}
        {tab === 'talk' && (
          <div className="flex min-h-0 flex-1 flex-col px-2">
            <TalkPanel onClose={() => setTab('analysis')} bottom={false} />
          </div>
        )}
      </div>
      <BottomBar
        onLeft={at > 0 ? () => setTab(TABS[at - 1].key) : undefined}
        right={last ? 'Ir para o Jogo' : 'Seguinte'}
        onRight={last ? play : () => setTab(TABS[at + 1].key)}
      />
    </div>
  )
}

/** A Análise durante a partida (o "Match Analysis" do FM08 continua no menu do jogo). */
export function AnalysisPane({ view }: { view: LiveView }) {
  const [a, setA] = useState<MatchAnalysis | null>(null)
  useEffect(() => {
    api.matchAnalysis().then(setA, () => setA(null))
  }, [])
  return a ? (
    <div className="flex flex-col py-2">
      <Analysis view={view} a={a} />
    </div>
  ) : null
}

/** O Pós-Jogo: o jornal a comparar a previsão com o resultado, os homens-chave e os marcos cumpridos. */
export function PostMatchPane({ view }: { view: LiveView }) {
  const [r, setR] = useState<MatchReview | null>(null)
  useEffect(() => {
    api.matchReview().then(setR, () => setR(null))
  }, [view.minute, view.finished])
  if (!r) return null
  const [home, away] = [view.home.club.shortName, view.away.club.shortName]
  const done = (side: 0 | 1, club: string) => (
    <Panel title={`Marcos Cumpridos · ${club}`} className="min-w-0 flex-1">
      <div className="cm-zebra py-1">
        {r.milestones[side].length === 0 ? (
          <div className="px-3 py-1 text-[14px] text-cm-dim">Não houve marcos para o {club} neste jogo.</div>
        ) : (
          r.milestones[side].map((m) => (
            <div key={m.id} className="flex h-[26px] items-center gap-3 px-3 text-[14px]">
              <span className="w-52 shrink-0 truncate font-semibold text-white">{m.name}</span>
              <span className="truncate text-cm-mint">{m.text}</span>
            </div>
          ))
        )}
      </div>
    </Panel>
  )
  return (
    <div className="flex flex-col gap-2 p-2">
      <Panel title="Análise Pós-Jogo">
        <div className="cm-zebra py-1">
          <Row label={`Análise do ${r.paper}`}>{r.analysis}</Row>
          <Row label={`Homem-Chave (${home})`}>{r.keyMen[0] ?? '-'}</Row>
          <Row label={`Homem-Chave (${away})`}>{r.keyMen[1] ?? '-'}</Row>
        </div>
      </Panel>
      <div className="flex gap-2">
        {done(0, home)}
        {done(1, away)}
      </div>
    </div>
  )
}
