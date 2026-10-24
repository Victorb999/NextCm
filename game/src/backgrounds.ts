// Fotos de fundo que vêm com o jogo (Opções Jogo → Fundo → Foto), em
// `assets/backgrounds/<grupo>/<nome>.webp`. O nome do ficheiro é a chave gravada nas definições.

const urls = import.meta.glob('./assets/backgrounds/*/*.webp', { eager: true, query: '?url', import: 'default' }) as Record<string, string>

export type BackgroundPhoto = { key: string; label: string; group: 'Estádios' | 'Momentos'; url: string }

const LABELS: Record<string, string> = {
  'estadios/anfield': 'Anfield',
  'estadios/stamford-bridge': 'Stamford Bridge',
  'estadios/maracana': 'Maracanã',
  'estadios/mineirao': 'Mineirão',
  'estadios/san-siro': 'San Siro',
  'estadios/san-siro-bancadas': 'San Siro (bancadas)',
  'estadios/signal-iduna-park': 'Signal Iduna Park',
  'estadios/signal-iduna-park-noite': 'Signal Iduna Park (noite)',
  'estadios/old-trafford': 'Old Trafford',
  'momentos/messi': 'Messi',
  'momentos/maradona-1986': 'Maradona, 1986',
  'momentos/mao-de-deus': 'Mão de Deus, 1986',
  'momentos/pele-1970': 'Pelé, 1970',
  'momentos/beckham-expulso': 'Beckham expulso, 1998',
  'momentos/dudek-istambul': 'Dudek em Istambul, 2005',
  'momentos/van-persie-2014': 'Van Persie, 2014',
  'momentos/brasil-a-correr': 'Brasil a correr',
  'momentos/bombonera': 'La Bombonera',
  'momentos/relvado-visto-do-ceu': 'Relvado visto do céu',
}

export const BACKGROUND_PHOTOS: BackgroundPhoto[] = Object.entries(urls)
  .map(([path, url]) => {
    const key = path.replace('./assets/backgrounds/', '').replace('.webp', '')
    return { key, label: LABELS[key] ?? key, group: key.startsWith('estadios/') ? 'Estádios' : 'Momentos', url } as BackgroundPhoto
  })
  .sort((a, b) => a.group.localeCompare(b.group) || a.label.localeCompare(b.label))

export const photoUrl = (key: string | null) => BACKGROUND_PHOTOS.find((p) => p.key === key)?.url
