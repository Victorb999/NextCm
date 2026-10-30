import { FileText } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../api'
import type { ContractReply } from '../bindings/ContractReply'
import type { ContractTerms } from '../bindings/ContractTerms'
import MoneySelect from '../components/MoneySelect'
import { BottomBar, ErrorBox, Link, Panel, ScreenHeader, Tabs } from '../components/Screen'
import { money, monthYear, wage as wageText, wagePeriod } from '../format'
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
      <div className="mx-1.5 flex min-h-0 flex-1 flex-col rounded-md bg-[#1a1b21] px-10 py-4">
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
  const btn = (text: string, on?: () => void, title?: string) =>
    on ? (
      <button className="cm-purple h-7 w-8 px-0" title={title} onClick={on}>
        {text}
      </button>
    ) : (
      <span className="w-8" />
    )
  return (
    <div className="flex h-9 items-center gap-1 px-4">
      <span className="w-56 shrink-0 text-[15px] font-semibold text-white">{label}</span>
      {btn('−', onMinus)}
      <span className="flex min-w-52 justify-center text-[15px] text-cm-yellow">{value}</span>
      {btn('+', onPlus)}
      {btn('=', onEqual, 'O que ele pede')}
    </div>
  )
}

function Proposal({ terms, onRole }: { terms: ContractTerms; onRole: () => void }) {
  const nav = useNav()
  const game = useGame()
  const period = wagePeriod()
  const [w, setW] = useState(terms.wage)
  const [bonus, setBonus] = useState(terms.bonus)
  const [years, setYears] = useState(terms.years)
  const [reply, setReply] = useState<ContractReply | null>(null)
  const [error, setError] = useState<string | null>(null)
  // O que ele pede agora (muda com a resposta)
  const asks = reply && !reply.accepted ? reply : terms
  const end = (y: number) => terms.ends[y - 1] ?? ''

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
        eyebrow={
          <Link onClick={() => nav.go({ kind: 'person', id: terms.personId })}>
            {terms.club ? `${terms.name} (${terms.club.shortName})` : terms.name}
          </Link>
        }
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
      <div className="mx-2 mt-2 flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
        <Panel title="Situação" className="shrink-0">
          <div className="px-4 py-2">
            <div className={`text-[16px] leading-snug ${reply?.accepted ? 'text-cm-yellow' : 'text-white'}`}>
              {reply && (reply.accepted || over) ? reply.text : wants}
            </div>
            <div className="mt-2 text-[14px] text-[#c8c8d0]">
              {terms.renewal
                ? `Contrato actual: ${wageText(terms.currentWage, true)} até ${monthYear(terms.currentExpires ?? '')}.`
                : terms.precontract
                  ? `Pré-contrato: o contrato dele acaba a ${monthYear(terms.currentExpires ?? '')} e chega livre nessa data.`
                  : terms.fee
                    ? `Transferência acordada: ${money(terms.fee)}.`
                    : 'Jogador livre.'}
            </div>
            <div className="mt-1 text-[14px] text-[#c8c8d0]">
              A sua direcção autoriza salários até {wageText(terms.maxWage, true)} e transferências até {money(terms.budget) || '€0'}.
            </div>
          </div>
        </Panel>
        <Panel title="Proposta" className="shrink-0">
          <div className="cm-zebra">
            <TermRow label="Função" value={ROLE_SHORT[terms.role] ?? terms.role} onPlus={over ? undefined : onRole} />
            <TermRow
              label={SALARY_LABEL[period.key] ?? 'Salário'}
              value={<MoneySelect value={w} factor={period.factor} onChange={setW} />}
              onEqual={() => setW(asks.wage)}
            />
            <TermRow
              label="Prémio Assinatura"
              value={<MoneySelect value={bonus} onChange={setBonus} />}
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
        </Panel>
        <ErrorBox>{error}</ErrorBox>
      </div>
      <BottomBar
        left={over ? 'Atrás' : 'Cancelar'}
        right={reply?.accepted || reply?.pending ? 'Continuar' : 'Oferecer'}
        onLeft={nav.back}
        onRight={reply?.accepted || reply?.pending ? () => nav.go({ kind: 'news' }) : over ? undefined : () => void offer()}
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
