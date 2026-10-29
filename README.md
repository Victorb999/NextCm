<div align="center">

# ⚽ NextCM

**O Championship Manager 01/02 que a gente ama, reconstruído com tecnologia moderna.**

O mesmo visual, a mesma velocidade, a mesma fluidez — com o banco de dados atualizado da época 2025/26.

![Rust](https://img.shields.io/badge/Rust-core-B7410E?logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)
![TypeScript](https://img.shields.io/badge/TypeScript-5A2A99?logo=typescript&logoColor=white)
![SQLite](https://img.shields.io/badge/SQLite-003B57?logo=sqlite&logoColor=white)
![Status](https://img.shields.io/badge/status-Fase%203%20·%20jogável-ffff40)

</div>

---

## 📖 Sobre

O **NextCM** recria a experiência do **Championship Manager 01/02** (com o tema *Coalizão* do patch CLZ) do zero: coluna esquerda, cabeçalhos com as cores do clube, abas escuras, botões roxos, menus e diálogos verdes, tudo instantâneo, com a densidade de informação do CM e um toque moderno (cantos levemente arredondados, fontes Inter e JetBrains Mono). Sem cara de dashboard: **a informação vem antes do estilo**.

O mundo do jogo vem dos arquivos `.dat` originais do CM 01/02 com o **CLZ Leagues Patch (DB 25-26)**: mais de **144 mil pessoas**, 11 mil clubes e 400 competições, extraídos para um SQLite. As regras do jogo vivem num núcleo em **Rust** determinístico — a mesma seed dá sempre a mesma época.

## ✨ O que já dá para fazer

### 🌍 Começar um jogo como no CM
- **Escolher Campeonato(s)**: cada nação jogável, de fundo (só resultados) ou fora do jogo.
- **Escolher Iniciar Época**: Europa 26/27 (1/7/2026) ou Brasil 2026 (1/1/2026).
- Gravar, carregar e apagar jogos (um SQLite por save, migrado sozinho), gravar no dia de jogo e Auto Gravação.

### ⚽ O mundo a jogar
- **Brasil** (Séries A a D, Copa do Brasil, estaduais), **Inglaterra, Espanha, Itália, Alemanha, França, Portugal, Holanda, Argentina e Arábia Saudita** (1ª e 2ª divisão), **Libertadores, Sul-Americana, Liga dos Campeões e Liga Europa**, com épocas de ano civil e de agosto a maio lado a lado.
- **Fim de época**: subidas e descidas, campeões, evolução e envelhecimento, contratos, reformas e juniores.
- **Mercado** nas janelas reais: a IA compra, vende, empresta e dispensa; finanças de cada clube.

### 🎮 O seu clube
- **Dia de jogo** como no CM: *Jogos de Hoje* → a partida ao vivo com relato minuto a minuto, estatísticas e notas → *Resultados de Hoje*; substituições e tácticas a meio do jogo.
- **Tácticas** arrastando jogadores no campo e na lista, setas de movimento, *Instruções à Equipa*, instruções individuais, banco de 9 e o adjunto que escolhe a equipa.
- **Mercado do treinador**: propostas com valor, prestações, percentagem de venda futura e extras, empréstimos, contratos com resposta e contraproposta, renovações, pré-contratos, lista de transferências.
- **Olheiros**: atributos tapados até conhecer o jogador, Observar Jogador, tarefas dos olheiros e as listas Olheiro #1–#7.
- **Plantel**: condição, moral, lesões e castigos, insatisfação e pedidos para sair, equipa de reservas, números de camisola, Disciplinar, elogiar e criticar na imprensa.
- **Direcção e Federação**: confiança, orçamentos, reuniões e pedidos, despedimento; Informação de Empregos, candidaturas e currículo do treinador.
- **Imprensa e palestras**: conferências antes e depois dos jogos e palestras aos jogadores, com a reação de cada um pela personalidade.

### 🔎 Enciclopédia
- **Pesquisa de jogadores e staff** em milissegundos, com os filtros do CM.
- **Perfil** com os atributos 1–20 exatamente como o CM mostra (a conversão foi lida do próprio `cm0102.exe`), contrato, transferência, historial e estatísticas por jogo.
- **Clubes, competições e nações** ligados entre si, com historial de campeões e títulos.

### 🎨 Personalização
- Moeda (€, £, $, R$), salários semanais/mensais/anuais, resolução, fontes, ecrã completo, cor da coluna lateral e fundo com a sua imagem.

## 🏗️ Arquitetura

```
MeuCM/
├─ core/            # nextcm-core (Rust): dados, calendário, match engine, tácticas, save
│  ├─ src/live.rs         # match engine por lances (relato, cartões, substituições)
│  ├─ src/bin/sim.rs      # CLI: simula épocas inteiras sem interface
│  └─ examples/dev_bridge.rs  # ponte HTTP para abrir a UI no navegador em dev
├─ game/            # interface React + Vite + Tailwind
│  └─ src-tauri/    # app desktop Tauri 2 (comandos que chamam o core)
├─ scripts/         # extrator Python dos .dat → data/world.sqlite
└─ ROADMAP.md       # plano, diretrizes e índice dos marcos (detalhe em docs/roadmap/)
```

| Camada | Tecnologia | Papel |
|---|---|---|
| Regras do jogo | **Rust** (`nextcm-core`) | Simulação, match engine com RNG seedado, escolha de equipa, save. Testável com `cargo test` e pela CLI, sem abrir a UI. |
| Desktop | **Tauri 2** | Expõe o core para a interface; os tipos TypeScript são gerados das structs Rust (`ts-rs`). |
| Interface | **React + TypeScript + Tailwind** | Só mostra dados e envia comandos — nenhuma regra de jogo no front. |
| Dados | **SQLite** | `world.sqlite` (o mundo) e um SQLite por save. |
| Extração | **Python** | Lê os `.dat` do CM 01/02 uma vez por patch. |

## 🚀 Como rodar

### Pré-requisitos
- [Rust](https://rustup.rs) (no Windows, com o linker do MSVC — *Visual Studio Build Tools*)
- [Node.js](https://nodejs.org) 20+
- Python 3
- Os arquivos `.dat` do **CLZ Leagues Patch (DB 25-26)** para o CM 01/02 (não estão no repositório — veja o aviso abaixo)

### 1. Gerar o mundo
Copie os `.dat` do patch para `data/raw/clz-25-26/` e rode:

```bash
python scripts/extract.py
```

Isso gera `data/world.sqlite`. Para conferir o resultado: `python scripts/validate.py`.

### 2. Abrir o jogo

```bash
cd game
npm install
npm run tauri dev
```

> **Windows:** rode o `cargo`/`tauri` pelo PowerShell — o `link` do Git Bash atrapalha o linker do MSVC.

### Desenvolvimento no navegador
Sem abrir o app desktop: suba a ponte de dev e o Vite (o Vite repassa `/invoke` para a ponte).

```bash
cargo run -p nextcm-core --example dev_bridge
npm --prefix game run dev
```

### Testes e simulação

```bash
powershell -File scripts/check.ps1                          # tudo: fmt, clippy, testes, tsc e oxlint
cargo test -p nextcm-core                                   # testes do core
cargo run -p nextcm-core --release --bin sim -- 7568 epoca 1 20   # 20 épocas da Série A
```

A CLI mostra a classificação, golos por jogo, vitórias em casa, remates, cartões, substituições e lesões por equipa, e a média de pontos e títulos de cada clube — é assim que o motor é calibrado.

## 🗺️ Roadmap

| | Marco | |
|---|---|---|
| ✅ | **Fase 1** | Extrator dos `.dat` → SQLite |
| ✅ | **Fase 2** | Enciclopédia navegável no visual do CM |
| ✅ | **3.1 – 3.4** | Novo jogo e tempo, Série A jogável, o seu time, dia de jogo com motor por lances |
| ✅ | **3.5 – 3.7** | Fim de época, mais competições no Brasil, as ligas estrangeiras e a Europa |
| ✅ | **3.8 – 3.10** | Mercado e finanças, personalidade, olheiros e conhecimento |
| ✅ | **3.11 – 3.14** | Direcção e Federação, empregos e imprensa, polimento, palestras |
| ⏳ | **3.15** | Testes e ajustes: jogar épocas inteiras, fluxos, o visual novo das telas, início do jogo como no CM |
| ⏳ | **3.16** | Plantel completo (Vêr, Filtrar, reservas e clubes B) |
| ⏳ | **3.17** | Treino |
| ⏳ | **3.18** | Editor de jogos salvos |

O plano está no [ROADMAP.md](ROADMAP.md); o detalhe de cada marco — decisões, regras e números — está em [docs/roadmap/](docs/roadmap/), um ficheiro por marco.

## ⚠️ Aviso

O NextCM é um **projeto de fã, sem fins lucrativos**, sem ligação com a Sports Interactive, a Eidos/Square Enix ou os autores do patch CLZ. *Championship Manager* é marca dos seus respetivos donos.

**Nenhum dado do jogo original ou do patch é distribuído aqui**: os `.dat`, o `world.sqlite` e os saves ficam fora do Git (ver `.gitignore`). Para jogar, é preciso ter os arquivos originais.

---

<div align="center">

Feito com ⚽ e saudade das noites em claro no CM 01/02.

</div>
