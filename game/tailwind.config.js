/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      // Paleta do CM 01/02 com o tema Coalizão (patch CLZ)
      colors: {
        cm: {
          screen: '#161616', // fundo das telas
          panel: '#0d0d0d', // painéis de lista
          line: '#2c2c2c', // divisórias
          // Coluna esquerda: personalizável em Opções Jogo (variáveis em settings.ts)
          side: 'var(--cm-side)',
          sidetop: 'var(--cm-side-top)',
          sidebtn: 'var(--cm-sidebtn)', // botões da coluna
          sidelight: 'var(--cm-sidelight)', // relevo claro
          sidedark: 'var(--cm-sidedark)', // relevo escuro
          sidemark: 'var(--cm-side-mark)', // "NEXTCM" na vertical
          tab: '#202020', // abas
          tabline: '#474747',
          purple: '#5b1fa6', // botões roxos / Atrás-Seguinte
          purplelight: '#8b5bd6',
          purpledim: '#8a63c4', // texto de botão roxo desabilitado
          head: '#5a2a99', // cabeçalho de tabela
          sel: '#9c0000', // linha selecionada
          yellow: '#ffff40',
          magenta: '#ff5cff',
          pink: '#ff77ff',
          orange: '#ff9a00',
          green: '#3ddc4a',
          pitch: '#0f5a16',
        },
      },
      fontFamily: {
        sans: ['var(--cm-text-font)'],
        title: ['var(--cm-title-font)'],
        // Rótulos em maiúsculas, cabeçalhos de colunas e números (como nas maquetas)
        mono: ['"JetBrains Mono"', 'Consolas', 'monospace'],
      },
      fontSize: {
        cm: ['13px', '18px'],
      },
    },
  },
  plugins: [],
}
