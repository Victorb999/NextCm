# 002 — Os botões afundam ao clicar

- **Status**: DONE
- **Commit**: c8786e4
- **Severity**: LOW
- **Category**: Missed opportunities
- **Estimated scope**: 1 ficheiro (`game/src/index.css`)

## Problem

`.cm-purple`, `.cm-side-btn`, `.cm-continue` e `.cm-tab` só mudam de cor ao carregar
(`game/src/index.css`: `.cm-purple:active:not(:disabled) { @apply bg-[#4a1888]; }`). O clique não tem tacto.

## Target

```css
.cm-purple, .cm-side-btn, .cm-continue, .cm-tab {
  transition: transform 100ms var(--ease-out);
}
.cm-purple:active:not(:disabled), .cm-side-btn:active:not(:disabled),
.cm-continue:active:not(:disabled), .cm-tab:active:not(:disabled) {
  transform: scale(0.97);
}
```

Só `transform` (nunca `transition: all`); a cor continua instantânea, como no CM.

## Boundaries

- Não animar hover nem cores. Não tocar nos menus de contexto nem nas linhas das tabelas.

## Verification

- **Mechanical**: `powershell -File scripts/check.ps1 -Quick`.
- **Feel check**: clicar depressa várias vezes num botão roxo; tem de responder sem atraso. A 10% de velocidade
  (DevTools > Animations) vê-se só um encolher de 3%.
