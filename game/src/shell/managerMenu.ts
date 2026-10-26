import type { GameInfo } from '../bindings/GameInfo'
import type { MenuNode } from '../components/CascadeMenu'
import type { Screen } from '../nav'

/** Os diálogos que o menu do treinador abre (em `ManagerDialogs`). */
export type ManagerDialog = 'save' | 'holiday' | 'resign' | 'retire'

/** O menu amarelo do treinador (botão com o nome dele na coluna). */
export function managerMenu({
  info,
  go,
  onExit,
  onDialog,
}: {
  info: GameInfo
  go: (s: Screen) => void
  onExit: () => void
  onDialog: (d: ManagerDialog) => void
}): MenuNode[] {
  const club = info.club
  return [
    { label: `Plantel ${club.shortName}`, onSelect: () => go({ kind: 'club', id: club.id }) },
    { label: `Reservas ${club.shortName}`, onSelect: () => go({ kind: 'club', id: club.id, reserves: true }) },
    { label: 'Números Plantel', onSelect: () => go({ kind: 'squadNumbers' }) },
    { label: 'Tácticas', onSelect: () => go({ kind: 'tactics', id: club.id }) },
    { label: 'Historial', onSelect: () => go({ kind: 'clubHistory', id: club.id }) },
    'separator',
    { label: info.unreadNews ? `Notícias (${info.unreadNews})` : 'Notícias', onSelect: () => go({ kind: 'news' }) },
    { label: 'Pesquisa Jogador & Staff', onSelect: () => go({ kind: 'search' }) },
    { label: 'Confiança da Direcção', onSelect: () => go({ kind: 'board' }) },
    { label: 'Informação de Empregos', onSelect: () => go({ kind: 'jobs' }) },
    { label: 'Transferências', onSelect: () => go({ kind: 'worldTransfers' }) },
    { label: 'Perfil do Treinador', onSelect: () => go({ kind: 'person', id: info.managerId }) },
    { label: 'Currículo', onSelect: () => go({ kind: 'career' }) },
    'separator',
    { label: 'Ir de Férias', onSelect: () => onDialog('holiday') },
    ...(info.sacked ? [] : [{ label: 'Rescindir com Clube', onSelect: () => onDialog('resign') }]),
    { label: 'Retirar-se', onSelect: () => onDialog('retire') },
    'separator',
    { label: 'Gravar Jogo', onSelect: () => onDialog('save') },
    { label: 'Sair para o Menu', onSelect: onExit },
  ]
}
