# 001 — Interruptores "Animações" e "Som" nas Opções

- **Status**: DONE
- **Commit**: c8786e4
- **Severity**: MEDIUM
- **Category**: Accessibility
- **Estimated scope**: 3 ficheiros, pequeno

## Problem

Não há forma de desligar o movimento nem nenhum `prefers-reduced-motion`. A linha "Som" de
`game/src/components/OptionsDialog.tsx:238` está desligada:

```tsx
<Row label="Som" disabled>
  <Option selected={false} disabled>Sim</Option>
  <Option selected disabled>Não</Option>
</Row>
```

## Target

- `Settings` (`game/src/settings.ts`) ganha `animations: boolean` (defeito `true`), `sound: boolean`
  (defeito `true`) e `volume: number` de 0 a 1 (defeito `0.5`).
- `applySettings` põe `document.documentElement.dataset.motion = s.animations ? 'on' : 'off'`.
- `game/src/index.css`: `--ease-out: cubic-bezier(0.23, 1, 0.32, 1);` em `:root`, e as animações anuladas com
  `[data-motion='off']` e com `@media (prefers-reduced-motion: reduce)`:
  `animation: none !important; transition: none !important;`.
- Opções Jogo: "Animações" (Sim/Não), "Som" (Sim/Não) e "Volume" (25%, 50%, 75%, 100%).

## Repo conventions to follow

- As opções Sim/Não usam `Option` como em `OptionsDialog.tsx` (ex.: a linha "Som" de hoje); as listas usam
  `Cycle` com `{ key, label }[]` como `AUTOSAVES` em `settings.ts`.
- Atributos no `<html>` como `dataset.bg` em `applySettings`.

## Steps

1. `settings.ts`: tipos, defaults, `VOLUMES` e o `dataset.motion` em `applySettings`.
2. `index.css`: token e o bloco que desliga.
3. `OptionsDialog.tsx`: trocar a linha "Som" desligada pelas três linhas.

## Boundaries

- Não mexer noutras opções. Sem dependências novas.

## Verification

- **Mechanical**: `npm --prefix game run check` sem erros.
- **Feel check**: nas Opções, desligar Animações e ver que os botões deixam de afundar ao clicar.
