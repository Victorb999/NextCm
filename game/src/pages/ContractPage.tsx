import { FileText } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ContractReply } from '../bindings/ContractReply'
import type { ContractTerms } from '../bindings/ContractTerms'
import { BottomBar, ScreenHeader, Tabs } from '../components/Screen'
import { currency, money, monthYear, moneyStep, wage as wageText, wagePeriod } from '../format'
import { useGame } from '../game'
import { useEscBack, useNav } from '../nav'

// Contrato com um jogador, como no CM: primeiro "Definir Função No Clube" e depois
// "Propôr Contrato" (salário, prémio de assinatura, fim do contrato). O jogador responde
// logo; o core decide e guarda tudo.

/** Nome curto de cada função, para a linha "Função" da proposta. */
const ROLE_SHORT: Record<string, string> = {
  key: 'Indispensável',
  important: 'Importante',
  rotation: 'Rotação',
  backup: 'Apoio',
  prospect: 'Perspectiva futura',
  youngster: 'Jovem com valor',
}

const SALARY_LABEL: Record<string, string> = { week: 'Salário Semanal', month: 'Salário Mensal', year: 'Salário Anual' }

/** "Este jogador é ..." — a lista de funções, com as vetadas pela idade a roxo. */
function RolePicker({ terms, onPick }: { terms: ContractTerms; onPick: (role: string) => void }) {
  const nav = useNav()
  const [selected, setSelected] = useState<string | null>(null)
  const vetoed = terms.roles.some((r) => !r.allowed)
  return (
    <>
      <ScreenHeader icon={<FileText size={22} />} eyebrow={terms.name} title="Definir Função No Clube" />
      <div className="mx-1.5 flex min-h-0 flex-1 flex-col border border-cm-line bg-cm-panel/70 px-10 py-4">
        <div className="pb-3 font-title text-[21px] text-white">Situação</div>
        {terms.roles.map((r) => (
          <button
            key={r.key}
            disabled={!r.allowed}
            onClick={() => setSelected(r.key)}
            onDoubleClick={() => r.allowed && onPick(r.key)}
            className={`ml-10 block text-left font-title text-[21px] leading-snug ${
              !r.allowed ? 'text-cm-purplelight' : r.key === selected ? 'text-cm-yellow' : 'text-white hover:text-cm-yellow'
            }`}
          >
            {r.youth && '+ '}
            {r.label}
          </button>
        ))}
        {vetoed && <div className="mt-8 font-title text-[21px] text-white">+ Vetado pela direcção devido à idade do jogador</div>}
      </div>
      <BottomBar right="Escolher" onLeft={nav.back} onRight={selected ? () => onPick(selected) : undefined} />
    </>
  )
}

/** Uma linha da proposta: nome, −, valor, +, = (o que ele pede). */
function TermRow({
  label,
  value,
  onMinus,
  onPlus,
  onEqual,
}: {
  label: string
  value: ReactNode
  onMinus?: () => void
  onPlus?: () => void
  onEqual?: () => void
}) {
  const btn = (text: string, on?: () => void) =>
    on ? (
      <button className="cm-chip h-6 w-7 text-[15px]" onClick={on}>
        {text}
      </button>
    ) : (
      <span className="w-7" />
    )
  return (
    <div className="flex h-[30px] items-center gap-2 font-title text-[20px]">
      <span className="w-72 shrink-0 text-white">{label}</span>
      {btn('−', onMinus)}
      <span className="w-48 text-center text-white">{value}</span>
      {btn('+', onPlus)}
      {btn('=', onEqual)}
    </div>
  )
}

/** Campo de dinheiro sem moldura (na moeda das Definições); `value` em euros × `factor`. */
function MoneyInput({ value, factor, onChange }: { value: number; factor: number; onChange: (eur: number) => void }) {
  const c = currency()
  const shown = Math.round(value * factor * c.rate)
  const [draft, setDraft] = useState<string | null>(null)
  return (
    <input
      value={draft ?? `${c.symbol}${shown.toLocaleString('pt-BR')}`}
      onFocus={() => setDraft(String(shown))}
      onChange={(e) => {
        const digits = e.target.value.replace(/\D/g, '')
        setDraft(digits)
        if (digits) onChange(Math.round(Number(digits) / factor / c.rate))
      }}
      onBlur={() => setDraft(null)}
      className="w-full bg-transparent text-center text-white outline-none focus:bg-black/60"
    />
  )
}

function Proposal({ terms, onRole }: { terms: ContractTerms; onRole: () => void }) {
  const nav = useNav()
  const game = useGame()
  const period = wagePeriod()
  const c = currency()
  const [w, setW] = useState(terms.wage)
  const [bonus, setBonus] = useState(terms.bonus)
  const [years, setYears] = useState(terms.years)
  const [reply, setReply] = useState<ContractReply | null>(null)
  const [error, setError] = useState<string | null>(null)
  // O que ele pede agora (muda com a resposta)
  const asks = reply && !reply.accepted ? reply : terms
  const end = (y: number) => terms.ends[y - 1] ?? ''
  const step = (eur: number, factor: number) => moneyStep(eur * factor * c.rate) / factor / c.rate

  const offer = async () => {
    try {
      // O − / + no período das Definições deixa casas decimais: o core recebe euros inteiros
      const r = await api.negotiate(terms.personId, terms.role, Math.round(w), Math.round(bonus), years)
      setReply(r)
      setError(null)
      const info = await api.gameInfo()
      if (info) game.update(info)
    } catch (e) {
      setError(String(e))
    }
  }

  // Negociação parada: rompida, fechada, ou a proposta seguiu e a resposta chega amanhã
  const over = (reply?.over ?? false) || (reply?.pending ?? false)
  const wants = `${terms.name} ${reply ? 'recusa: quer' : 'quer'} ${wageText(asks.wage, true)} até ${monthYear(end(asks.years))} mais um prémio de assinatura no valor de ${money(asks.bonus) || '€0'}.`
  return (
    <>
      <ScreenHeader
        club={terms.club}
        icon={!terms.club && <FileText size={22} />}
        eyebrow={terms.club ? `${terms.name} (${terms.club.shortName})` : terms.name}
        title="Propôr Contrato"
      />
      <Tabs
        tabs={[
          { key: 'basic', label: 'Básico' },
          { key: 'bonuses', label: 'Prémios', disabled: true },
          { key: 'clauses', label: 'Cláusulas', disabled: true },
        ]}
        active="basic"
        onChange={() => {}}
      />
      <div className="mx-1.5 mt-1.5 flex min-h-0 flex-1 flex-col overflow-auto border border-cm-line bg-cm-panel/70 px-5 py-3">
        <div className={`font-title text-[21px] leading-snug ${reply?.accepted ? 'text-cm-yellow' : 'text-white'}`}>
          {reply && (reply.accepted || over) ? reply.text : wants}
        </div>
        <div className="mt-3 font-title text-[19px] text-white">
          {terms.renewal
            ? `Contrato actual: ${wageText(terms.currentWage, true)} até ${monthYear(terms.currentExpires ?? '')}.`
            : terms.precontract
              ? `Pré-contrato: o contrato dele acaba a ${monthYear(terms.currentExpires ?? '')} e chega livre nessa data.`
              : terms.fee
                ? `Transferência acordada: ${money(terms.fee)}.`
                : 'Jogador livre.'}
        </div>
        <div className="mt-1 font-title text-[19px] text-white">
          A sua direcção autoriza salários até {wageText(terms.maxWage, true)} e transferências até {money(terms.budget) || '€0'}.
        </div>
        <div className="mt-5 pl-6">
          <TermRow label="Função" value={ROLE_SHORT[terms.role] ?? terms.role} onPlus={over ? undefined : onRole} />
          <TermRow
            label={SALARY_LABEL[period.key] ?? 'Salário'}
            value={<MoneyInput value={w} factor={period.factor} onChange={setW} />}
            onMinus={() => setW((v) => Math.max(0, v - step(v, period.factor)))}
            onPlus={() => setW((v) => v + step(v, period.factor))}
            onEqual={() => setW(asks.wage)}
          />
          <TermRow
            label="Prémio Assinatura"
            value={<MoneyInput value={bonus} factor={1} onChange={setBonus} />}
            onMinus={() => setBonus((v) => Math.max(0, v - step(v, 1)))}
            onPlus={() => setBonus((v) => v + step(Math.max(v, 10_000), 1))}
            onEqual={() => setBonus(asks.bonus)}
          />
          <TermRow
            label="Final Contrato"
            value={monthYear(end(years))}
            onMinus={years > 1 ? () => setYears(years - 1) : undefined}
            onPlus={years < 5 ? () => setYears(years + 1) : undefined}
            onEqual={() => setYears(asks.years)}
          />
          <TermRow label="Tipo de Contrato" value="Tempo Inteiro" />
          <TermRow label="Tornar proposta pública" value="Não" />
        </div>
        {error && <div className="mt-3 font-title text-[19px] font-bold text-[#ff5050]">{error}</div>}
      </div>
      <BottomBar
        left={over ? 'Atrás' : 'Cancelar'}
        right={reply?.accepted || reply?.pending ? 'Continuar' : 'Oferecer'}
        onLeft={nav.back}
        onRight={reply?.accepted || reply?.pending ? nav.back : over ? undefined : () => void offer()}
      />
    </>
  )
}

export default function ContractPage({ id }: { id: number }) {
  useEscBack()
  const [terms, setTerms] = useState<ContractTerms | null>(null)
  const [role, setRole] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.contractTerms(id, role).then(setTerms, (e) => setError(String(e)))
  }, [id, role])

  if (error) return <div className="p-6 text-center font-title text-xl text-cm-magenta">{error}</div>
  if (!terms) return null
  return (
    <div className="flex h-full flex-col">
      {role === null ? (
        <RolePicker terms={terms} onPick={setRole} />
      ) : (
        // A chave refaz a proposta com o que ele pede pela função escolhida
        <Proposal key={`${terms.role}`} terms={terms} onRole={() => setRole(null)} />
      )}
    </div>
  )
}
