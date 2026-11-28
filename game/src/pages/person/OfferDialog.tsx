import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../../api'
import type { DealTerms } from '../../bindings/DealTerms'
import type { ExchangeCandidate } from '../../bindings/ExchangeCandidate'
import type { LoanTerms } from '../../bindings/LoanTerms'
import type { TransferView } from '../../bindings/TransferView'
import { Choice, ExchangePicker, MoneyCell } from '../../components/DealDialogs'
import { DialogFrame, Overlay } from '../../components/Dialog'
import type { ExchangeOption } from '../../components/DealDialogs'
import { ErrorBox, Panel } from '../../components/Screen'
import { money, wage } from '../../format'
import { useGame } from '../../game'
import {
  DEADLINES,
  FEE_PCT,
  INSTALMENTS,
  LOAN_LENGTH,
  LOAN_TERMS,
  NO_TERMS,
  SELL_ON,
  WAGE_SHARE,
  budgetText,
  nextCaps,
  nextGames,
} from '../../transferTerms'

// A proposta por um jogador de outro clube, separada em painéis como a "Transfer Offer" do FM08
// e o perfil: à esquerda a Proposta Básica (transferência ou empréstimo), os valores adicionais,
// as cláusulas e a troca; à direita a visão geral e os comentários.

export type OfferKind = 'offer' | 'loan'

const YES_NO: [boolean, string][] = [
  [false, 'Não'],
  [true, 'Sim'],
]
const KINDS: [OfferKind, string][] = [
  ['offer', 'Transferência'],
  ['loan', 'Empréstimo'],
]

/** Uma linha de um painel: o rótulo à esquerda e os controlos à direita. */
function Row({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="flex min-h-10 items-center gap-3 px-3">
      <span className="w-64 shrink-0 text-[15px] font-semibold text-white">{label}</span>
      <div className="flex flex-1 items-center gap-1">{children}</div>
    </div>
  )
}

export default function OfferDialog({
  view,
  name,
  kind: startKind,
  deal,
  loan: startLoan,
  onCancel,
  onSent,
}: {
  view: TransferView
  name: string
  kind: OfferKind
  /** As condições de uma proposta anterior ("Nova Proposta"). */
  deal?: DealTerms
  loan?: LoanTerms
  onCancel: () => void
  onSent: (v: TransferView) => void
}) {
  const game = useGame()
  const first = deal ?? { ...NO_TERMS, fee: Math.min(view.value, view.budget) || view.value }
  const [kind, setKind] = useState<OfferKind>(startKind)
  const [t, setT] = useState<DealTerms>(first)
  const [l, setL] = useState<LoanTerms>(startLoan ?? LOAN_TERMS)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  // A troca: o plantel do treinador e quem dele o clube vendedor aceita (decide o core)
  const [squad, setSquad] = useState<ExchangeOption[]>([])
  const [exchangeable, setExchangeable] = useState<ExchangeCandidate[] | null>(null)
  const [picking, setPicking] = useState(false)
  const club = game.info?.club.id
  const person = view.personId
  useEffect(() => {
    if (!club) return
    api.getClub(club).then(
      (c) => setSquad(c.players.map((p) => ({ id: p.id, name: p.listName, position: p.position, age: p.age, wage: p.wage }))),
      () => {},
    )
    api.exchangeCandidates(person).then(setExchangeable, () => setExchangeable([]))
  }, [club, person])

  const set = (p: Partial<DealTerms>) => setT((old) => ({ ...old, ...p }))
  const setLoan = (p: Partial<LoanTerms>) => setL((old) => ({ ...old, ...p }))
  const reset = () => {
    setT(first)
    setL(startLoan ?? LOAN_TERMS)
    setError(null)
  }

  // Emprestado ao clube do treinador: o vendedor é o clube dono
  const seller = (view.club?.id === club && view.loanFrom ? view.loanFrom : view.club)?.shortName ?? ''
  const kinds = KINDS.filter(([k]) => (k === 'offer' ? view.canOffer : view.canLoan) || k === kind)
  const isLoan = kind === 'loan'

  // O que o jogador da troca vale ao clube: sai do valor da transferência (e volta ao tirá-lo)
  const worth = (id: number | null) => (id === null ? 0 : (exchangeable?.find((c) => c.id === id)?.worth ?? 0))
  const swap = (id: number | null) => set({ exchange: id, fee: Math.max(0, t.fee + worth(t.exchange) - worth(id)) })
  const exchange = squad.find((p) => p.id === t.exchange)

  // A taxa mensal do empréstimo em % do salário do mês (o core recebe o valor em euros)
  const monthly = (view.wage * 52) / 12
  const feeOf = (pct: number) => Math.round((monthly * pct) / 100)
  const feeOptions: [number, string][] = FEE_PCT.map((p): [number, string] => [feeOf(p), p === 0 ? '0%' : `${p}% (${money(feeOf(p))})`])
  if (!feeOptions.some((o) => o[0] === l.monthlyFee)) {
    const p = monthly > 0 ? Math.round((l.monthlyFee / monthly) * 100) : 0
    feeOptions.push([l.monthlyFee, `${p}% (${money(l.monthlyFee)})`])
    feeOptions.sort((a, b) => a[0] - b[0])
  }

  const send = async () => {
    if (busy) return
    setBusy(true)
    try {
      onSent(await (isLoan ? api.askLoan(person, l) : api.makeOffer(person, t)))
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  // Enter envia e Esc volta (com a lista da troca aberta, é ela que responde)
  useEffect(() => {
    if (picking) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation()
        onCancel()
      } else if (e.key === 'Enter') {
        e.stopImmediatePropagation()
        void send()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  })

  const games = (
    <button
      className="underline decoration-dotted underline-offset-4 hover:text-cm-yellow"
      title="Mudar o número de jogos"
      onClick={() => set({ appsNeeded: nextGames(t.appsNeeded || 10) })}
    >
      {t.appsNeeded || 10}
    </button>
  )
  const caps = (
    <button
      className="underline decoration-dotted underline-offset-4 hover:text-cm-yellow"
      title="Mudar o número de jogos pela seleção"
      onClick={() => set({ capsNeeded: nextCaps(t.capsNeeded || 5) })}
    >
      {t.capsNeeded || 5}
    </button>
  )
  const total = t.fee + t.appsBonus + t.capsBonus

  return (
    <Overlay>
      <DialogFrame
        title={`${isLoan ? 'Empréstimo de' : 'Proposta por'} ${name}`}
        width={1180}
        onClose={onCancel}
        footer={
          <>
            <button className="cm-purple h-9 flex-1 font-title text-xl" onClick={onCancel}>
              Cancelar
            </button>
            <button className="cm-purple h-9 flex-1 font-title text-xl" onClick={reset}>
              Desfazer Alterações
            </button>
            <button className="cm-purple h-9 flex-1 font-title text-xl enabled:text-cm-yellow" disabled={busy} onClick={() => void send()}>
              {busy ? 'Enviando…' : isLoan ? 'Pedir Empréstimo' : 'Fazer Proposta'}
            </button>
          </>
        }
      >
        <div className="flex items-start gap-2">
          {/* À esquerda, a proposta */}
          <div className="flex min-w-0 flex-[1.25] flex-col gap-2">
            <Panel title="Proposta Básica">
              <div className="cm-zebra">
                {kinds.length > 1 && (
                  <Row label="Tipo de Proposta">
                    <Choice value={kind} options={kinds} onChange={setKind} />
                  </Row>
                )}
                {isLoan ? (
                  <>
                    <Row label="Duração">
                      <Choice value={l.months} options={LOAN_LENGTH} onChange={(months) => setLoan({ months })} />
                    </Row>
                    <Row label="Salário Pago">
                      <Choice value={l.wageShare} options={WAGE_SHARE} onChange={(wageShare) => setLoan({ wageShare })} />
                    </Row>
                    <Row label="Taxa Mensal">
                      <Choice value={l.monthlyFee} options={feeOptions} onChange={(monthlyFee) => setLoan({ monthlyFee })} />
                      <button
                        className="cm-purple h-7 w-8"
                        title={`Taxa que cobre o resto do salário (${Math.max(0, 100 - l.wageShare)}%)`}
                        disabled={monthly <= 0}
                        onClick={() => setLoan({ monthlyFee: feeOf(Math.max(0, 100 - l.wageShare)) })}
                      >
                        =
                      </button>
                    </Row>
                    <Row label="Opção de Compra">
                      <MoneyCell value={l.optionFee} onChange={(optionFee) => setLoan({ optionFee })} />
                    </Row>
                  </>
                ) : (
                  <>
                    <Row label="Valor Transferência">
                      <MoneyCell value={t.fee} onChange={(fee) => set({ fee })} />
                      <button className="cm-purple h-7 w-8" title="O valor do jogador" onClick={() => set({ fee: view.value })}>
                        =
                      </button>
                    </Row>
                    <Row label="Negociável">
                      <Choice value={!t.firm} options={YES_NO} onChange={(v) => set({ firm: !v })} />
                    </Row>
                    <Row label="Prazo da Decisão">
                      <Choice value={t.deadline} options={DEADLINES} onChange={(deadline) => set({ deadline })} />
                    </Row>
                    <Row label="Tornar Proposta Pública">
                      <Choice value={t.public} options={YES_NO} onChange={(v) => set({ public: v })} />
                    </Row>
                  </>
                )}
              </div>
            </Panel>
            {!isLoan && (
              <>
                <Panel title="Valores Adicionais">
                  <div className="cm-zebra">
                    <Row label="Parcelas Mensais">
                      <Choice value={t.months} options={INSTALMENTS} onChange={(months) => set({ months })} />
                    </Row>
                    <Row label={<>Valor após {games} jogos no campeonato</>}>
                      <MoneyCell value={t.appsBonus} onChange={(appsBonus) => set({ appsBonus })} />
                    </Row>
                    <Row label={<>Valor após {caps} jogos pela seleção</>}>
                      <MoneyCell value={t.capsBonus} onChange={(capsBonus) => set({ capsBonus })} />
                    </Row>
                  </div>
                </Panel>
                <Panel title="Cláusulas">
                  <Row label="Porcentagem da Próxima Venda">
                    <Choice value={t.sellOn} options={SELL_ON} onChange={(sellOn) => set({ sellOn })} />
                  </Row>
                </Panel>
                <Panel title="Jogadores na Troca">
                  <div className="flex min-h-10 items-center gap-1 px-3">
                    <button className="cm-purple h-7 w-32" onClick={() => setPicking(true)}>
                      Escolher
                    </button>
                    <button className="cm-purple h-7 w-32" disabled={!exchange} onClick={() => swap(null)}>
                      Remover
                    </button>
                    <span className="ml-3 truncate text-[15px] text-cm-yellow">
                      {exchange ? (
                        `${exchange.name} (${exchange.position}) vai na troca: −${money(worth(exchange.id)) || '€0'}`
                      ) : (
                        <span className="text-cm-purplelight">Nenhum jogador na troca</span>
                      )}
                    </span>
                  </div>
                </Panel>
              </>
            )}
          </div>

          {/* À direita, a visão geral e os comentários */}
          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <Panel title="Visão Geral">
              <div className="px-3 py-2.5 text-[15px] leading-relaxed text-white">
                {name}, {seller}, está avaliado em <span className="text-cm-yellow">{money(view.value) || '€0'}</span>
                {isLoan ? (
                  <>
                    {' '}
                    e ganha <span className="text-cm-yellow">{wage(view.wage, true)}</span>.
                  </>
                ) : (
                  '.'
                )}
                <div className="mt-1 text-cm-muted">{budgetText(view)}</div>
              </div>
            </Panel>
            <Panel title="Comentários">
              <ul className="px-3 py-2.5 text-[15px] leading-snug text-cm-green">
                {isLoan ? (
                  <>
                    <li>- Sem opção de compra, deixe o valor em zero.</li>
                    <li>- O {seller} responde dentro de alguns dias.</li>
                  </>
                ) : (
                  <>
                    <li>- Valor total da transferência: {money(total) || '€0'}.</li>
                    {t.firm && <li>- Não negociável: o {seller} aceita ou recusa, sem contraproposta.</li>}
                    {t.deadline === 1 && <li>- Com prazo de amanhã, o {seller} decide sob pressão e recusa mais vezes.</li>}
                    {view.comments.map((c) => (
                      <li key={c}>- {c}</li>
                    ))}
                  </>
                )}
              </ul>
            </Panel>
            <ErrorBox>{error}</ErrorBox>
          </div>
        </div>
      </DialogFrame>
      {picking && (
        <ExchangePicker
          club={seller}
          squad={squad}
          exchangeable={exchangeable}
          current={t.exchange}
          onCancel={() => setPicking(false)}
          onPick={(id) => {
            swap(id)
            setPicking(false)
          }}
        />
      )}
    </Overlay>
  )
}
