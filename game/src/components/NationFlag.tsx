import { FLAGS } from '../flags'

/** A bandeira de uma nação (pelo nome do banco), à altura do texto; nada se não houver. */
export default function NationFlag({ name, className = '' }: { name: string | null | undefined; className?: string }) {
  const code = name ? FLAGS[name] : undefined
  if (!code) return null
  return <span className={`fi fi-${code} shrink-0 rounded-[2px] shadow-[0_0_0_1px_rgba(0,0,0,0.5)] ${className}`} aria-hidden="true" />
}
