import { useNav } from '../nav'
import BoardPage from '../pages/BoardPage'
import ClubHistoryPage from '../pages/ClubHistoryPage'
import ClubPage from '../pages/ClubPage'
import CompetitionPage from '../pages/CompetitionPage'
import CompetitionsPage from '../pages/CompetitionsPage'
import FriendlyPage from '../pages/FriendlyPage'
import ContractPage from '../pages/ContractPage'
import JobsPage from '../pages/JobsPage'
import NationPage from '../pages/NationPage'
import NationsPage from '../pages/NationsPage'
import NewsPage from '../pages/NewsPage'
import PersonPage from '../pages/PersonPage'
import PressPage from '../pages/PressPage'
import ComparePage from '../pages/ComparePage'
import WorldTransfersPage from '../pages/WorldTransfersPage'
import CareerPage from '../pages/CareerPage'
import SearchPage from '../pages/SearchPage'
import SquadNumbersPage from '../pages/SquadNumbersPage'
import TacticsPage from '../pages/TacticsPage'
import TrainingPage from '../pages/TrainingPage'
import MatchSheetPage from '../pages/MatchSheetPage'
import { useHistoryKeys } from './useHistoryKeys'

/** A tela atual do histórico de navegação (`nav.current`). */
export default function Screens() {
  const { current } = useNav()
  useHistoryKeys()

  return (
    <>
      {/* A pesquisa fica montada para manter filtros, página e seleção ao voltar */}
      <div className={current.kind === 'search' ? 'h-full' : 'hidden'}>
        <SearchPage active={current.kind === 'search'} />
      </div>
      {current.kind === 'news' && <NewsPage key={String(current.latest)} latest={current.latest} />}
      {current.kind === 'person' && <PersonPage key={current.id} id={current.id} tab={current.tab} />}
      {current.kind === 'contract' && <ContractPage key={current.id} id={current.id} />}
      {current.kind === 'squadNumbers' && <SquadNumbersPage />}
      {current.kind === 'training' && <TrainingPage />}
      {current.kind === 'match' && <MatchSheetPage key={current.id} id={current.id} />}
      {current.kind === 'board' && <BoardPage />}
      {current.kind === 'jobs' && <JobsPage />}
      {current.kind === 'press' && <PressPage key={current.id} id={current.id} />}
      {current.kind === 'worldTransfers' && <WorldTransfersPage />}
      {current.kind === 'career' && <CareerPage />}
      {current.kind === 'compare' && <ComparePage key={`${current.a}-${current.b}`} a={current.a} b={current.b} />}
      {current.kind === 'club' && (
        <ClubPage
          key={`${current.id}-${current.reserves ?? false}-${current.tab ?? ''}`}
          id={current.id}
          reserves={current.reserves}
          initialTab={current.tab}
        />
      )}
      {current.kind === 'tactics' && <TacticsPage key={current.id} id={current.id} />}
      {current.kind === 'clubHistory' && <ClubHistoryPage key={current.id} id={current.id} />}
      {current.kind === 'competitions' && <CompetitionsPage />}
      {current.kind === 'competition' && (
        <CompetitionPage key={`${current.id}-${current.tab ?? ''}`} id={current.id} initialTab={current.tab} />
      )}
      {current.kind === 'nations' && <NationsPage />}
      {current.kind === 'nation' && <NationPage key={current.id} id={current.id} />}
      {current.kind === 'friendly' && <FriendlyPage key={current.mode} mode={current.mode} />}
    </>
  )
}
