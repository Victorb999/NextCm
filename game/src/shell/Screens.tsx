import { useNav } from '../nav'
import BoardPage from '../pages/BoardPage'
import ClubHistoryPage from '../pages/ClubHistoryPage'
import ClubPage from '../pages/ClubPage'
import CompetitionPage from '../pages/CompetitionPage'
import CompetitionsPage from '../pages/CompetitionsPage'
import ContractPage from '../pages/ContractPage'
import JobsPage from '../pages/JobsPage'
import NationPage from '../pages/NationPage'
import NationsPage from '../pages/NationsPage'
import NewsPage from '../pages/NewsPage'
import PersonPage from '../pages/PersonPage'
import SearchPage from '../pages/SearchPage'
import SquadNumbersPage from '../pages/SquadNumbersPage'
import TacticsPage from '../pages/TacticsPage'
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
      {current.kind === 'news' && <NewsPage />}
      {current.kind === 'person' && <PersonPage key={current.id} id={current.id} />}
      {current.kind === 'contract' && <ContractPage key={current.id} id={current.id} />}
      {current.kind === 'squadNumbers' && <SquadNumbersPage />}
      {current.kind === 'board' && <BoardPage />}
      {current.kind === 'jobs' && <JobsPage />}
      {current.kind === 'club' && <ClubPage key={`${current.id}-${current.reserves ?? false}`} id={current.id} reserves={current.reserves} />}
      {current.kind === 'tactics' && <TacticsPage key={current.id} id={current.id} />}
      {current.kind === 'clubHistory' && <ClubHistoryPage key={current.id} id={current.id} />}
      {current.kind === 'competitions' && <CompetitionsPage />}
      {current.kind === 'competition' && <CompetitionPage key={`${current.id}-${current.tab ?? ''}`} id={current.id} initialTab={current.tab} />}
      {current.kind === 'nations' && <NationsPage />}
      {current.kind === 'nation' && <NationPage key={current.id} id={current.id} />}
    </>
  )
}
