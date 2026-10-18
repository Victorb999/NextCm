// Continentes do banco (ids 0–5) com os nomes em português.

export const CONTINENTS: { id: number; label: string }[] = [
  { id: 5, label: 'América do Sul' },
  { id: 2, label: 'Europa' },
  { id: 3, label: 'América do Norte' },
  { id: 0, label: 'África' },
  { id: 1, label: 'Ásia' },
  { id: 4, label: 'Oceânia' },
]

const BY_NAME: Record<string, string> = {
  Africa: 'África',
  Asia: 'Ásia',
  Europe: 'Europa',
  'North America': 'América do Norte',
  Oceania: 'Oceânia',
  'South America': 'América do Sul',
}

export const continentPt = (name: string | null | undefined) => (name ? (BY_NAME[name] ?? name) : '-')

// Último continente escolhido em cada tela, para manter ao voltar
const remembered = new Map<string, string>()
export const rememberTab = (screen: string, tab?: string) => {
  if (tab !== undefined) remembered.set(screen, tab)
  return remembered.get(screen)
}
