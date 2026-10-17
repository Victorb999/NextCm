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
          side: '#45526a', // coluna esquerda
          sidebtn: '#39455a', // botões da coluna
          sidelight: '#8792a7', // relevo claro
          sidedark: '#1d2430', // relevo escuro
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
        sans: ['Tahoma', 'Verdana', 'sans-serif'],
        title: ['"Trebuchet MS"', 'Tahoma', 'sans-serif'],
      },
      fontSize: {
        cm: ['13px', '18px'],
      },
    },
  },
  plugins: [],
}
