import { api } from '../api'
import type { GameInfo } from '../bindings/GameInfo'
import type { MenuNode } from '../components/CascadeMenu'
import type { Screen } from '../nav'

/** O menu amarelo do treinador (botão com o nome dele na coluna). */
export function managerMenu({
  info,
  go,
  update,
  notify,
  onExit,
}: {
  info: GameInfo
  go: (s: Screen) => void
  update: (info: GameInfo) => void
  notify: (msg: string) => void
  onExit: () => void
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
    { label: 'Perfil do Treinador', onSelect: () => go({ kind: 'person', id: info.managerId }) },
    'separator',
    {
      label: 'Gravar Jogo',
      onSelect: () =>
        void api.saveGame().then(
          (g) => {
            update(g)
            notify('Jogo gravado.')
          },
          (e) => notify(String(e)),
        ),
    },
    { label: 'Sair para o Menu', onSelect: onExit },
  ]
}
