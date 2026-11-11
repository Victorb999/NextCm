import type { PersonProfile } from './bindings/PersonProfile'
import type { TransferView } from './bindings/TransferView'
import { compareFirst } from './compare'

// As acções de um jogador, iguais no menu Acção do perfil e no menu do botão direito das listas
// (como no FM). Quem pode o quê decide o core (`TransferView`, `PersonProfile`); aqui só se
// montam os grupos. A acção escolhida numa lista abre o perfil e corre lá (`pendingAct`).

export type PersonAction = { key: string; label: string }

/** Os grupos do menu (vazios ficam de fora); o último é o de todos: alcunha, nota, comparar. */
export function personActions({
  person,
  market,
  mine,
  playing,
}: {
  person: PersonProfile
  market: TransferView | null
  mine: boolean
  playing: boolean
}): PersonAction[][] {
  const m = market
  const canScout = playing && !!person.player && !mine && !person.reportOn
  // Emprestado por outro clube: não é nosso para vender, emprestar ou dispensar
  const loanedIn = !!m?.loanFrom
  const groups: PersonAction[][] = mine
    ? [
        // Mercado. Emprestado ao clube: compra-se ao clube dono (o core diz se dá)
        loanedIn
          ? m?.canOffer
            ? [{ key: 'offer', label: 'Abordar Para Comprar' }]
            : []
          : [
              ...(m?.canRenew ? [{ key: 'renew', label: 'Oferecer Novo Contrato' }] : []),
              m?.transferListed
                ? { key: 'unlist', label: 'Retirar da Lista de Transferências' }
                : { key: 'list', label: 'Colocar na Lista de Transferências' },
              m?.loanListed
                ? { key: 'unloanlist', label: 'Retirar da Lista de Empréstimos' }
                : { key: 'loanlist', label: 'Colocar na Lista de Empréstimos' },
              ...(m?.transferListed ? [{ key: 'price', label: 'Definir Preço Pedido' }] : []),
              ...(m?.windowOpen && !m.offer ? [{ key: 'shop', label: 'Oferecer a Clubes' }] : []),
            ],
        // Plantel: reservas ou equipa B, pelo que o core diz
        [
          ...(person.squadMove ? [{ key: person.squadMove.key, label: person.squadMove.label }] : []),
          // Saudades de casa: duas semanas no seu país
          ...(person.status?.canLeave ? [{ key: 'leave', label: 'Dar Folga' }] : []),
          // A função sem mexer no contrato (os emprestados por outros não têm)
          ...(loanedIn ? [] : [{ key: 'role', label: 'Definir Função no Plantel' }]),
          { key: 'discipline', label: 'Disciplinar Jogador' },
          ...(loanedIn ? [] : [{ key: 'release', label: 'Dispensar' }]),
        ],
        // Imprensa
        [
          { key: 'praise', label: 'Elogiar na Imprensa' },
          { key: 'criticise', label: 'Criticar na Imprensa' },
        ],
        // Staff
        [
          { key: 'medical', label: 'Pedir Relatório Médico' },
          { key: 'fitness', label: 'Pedir Relatório Preparador Físico' },
        ],
      ]
    : [
        // Emprestado pelo clube do treinador a outro: o contrato e chamá-lo de volta (o core diz se dá)
        [
          ...(m?.canRenew ? [{ key: 'renew', label: 'Oferecer Novo Contrato' }] : []),
          ...(m?.canRecall ? [{ key: 'recall', label: 'Chamar de Volta do Empréstimo' }] : []),
        ],
        [
          ...(m?.canOffer && !m.free ? [{ key: 'offer', label: 'Abordar Para Comprar' }] : []),
          ...(m?.canLoan ? [{ key: 'loan', label: 'Propôr Empréstimo' }] : []),
          ...(m?.canOption ? [{ key: 'option', label: 'Exercer Opção de Compra' }] : []),
          ...(person.canContract ? [{ key: 'contract', label: 'Propor Contrato' }] : []),
        ],
        // Da equipa B do treinador: Promover à Equipa 'A'
        person.squadMove ? [{ key: person.squadMove.key, label: person.squadMove.label }] : [],
        [
          person.shortlisted
            ? { key: 'unshortlist', label: 'Retirar da Lista Preferencial' }
            : { key: 'shortlist', label: 'Adicionar à Lista Preferencial' },
          ...(canScout ? [{ key: 'scout', label: 'Observar Jogador' }] : []),
        ],
      ]
  // Para todos: alcunha, nota e comparar (o 1º escolhido espera pelo 2º)
  const first = compareFirst()
  const common: PersonAction[] = [
    { key: 'nickname', label: 'Definir Alcunha' },
    { key: 'note', label: 'Criar Nota Treinador' },
    first && first.id !== person.id
      ? { key: 'compare', label: `Comparar com ${first.name}` }
      : { key: 'compareFirst', label: 'Comparar Jogadores' },
  ]
  return [...groups, common].filter((g) => g.length > 0)
}

// A acção escolhida no botão direito de uma lista, à espera de o perfil abrir
let pending: { id: number; key: string } | null = null

export const setPendingAct = (id: number, key: string) => {
  pending = { id, key }
}

/** A acção pendente deste jogador (e esquece-a: voltar ao perfil não a repete). */
export function takePendingAct(id: number): string | null {
  const p = pending
  pending = null
  return p?.id === id ? p.key : null
}
