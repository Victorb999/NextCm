# Modelo das notas da Release

Copiar para a descrição da Release no GitHub e trocar a `<versão>` e as novidades. O link do site e o
crédito à Coalizão CM vão sempre (marco 3.23).

---

Novo beta do NextCM. Baixe o **NextCM_<versão>_x64-setup.exe** abaixo, em Assets.

**[Site do jogo](https://victorb999.github.io/NextCm/)**: prints, funcionalidades e o download da versão mais recente.

A base de dados 2025/26 e o tema vêm do **Update Coalizão**, da equipe
[Coalizão CM](https://www.coalizaocm.com.br/), o maior acervo nacional de CM 01-02. Vale a visita.

## Novidades

- …

## Requisitos

- Windows 10 ou 11 (64 bits)
- Ecrã de pelo menos **1280×720** (a janela tem tamanho fixo)
- ~300 MB livres

## Instalar

1. Baixe `NextCM_<versão>_x64-setup.exe` abaixo, em **Assets**.
2. Abra o instalador. Não pede administrador: instala só para o seu utilizador.
3. **O Windows pode dizer "O Windows protegeu o computador".** O instalador não tem
   certificado pago, por isso o SmartScreen avisa. Clique em **Mais informações → Executar assim mesmo**.
4. Se faltar o WebView2 (raro no Windows 10/11 atualizado), o instalador baixa-o sozinho.

A base de dados já vem dentro do instalador: não precisa do CM 01/02 nem de extrair nada.

Para atualizar para um beta novo, basta instalar por cima. Para desinstalar: Definições do Windows → Aplicações → NextCM.

## Onde ficam as coisas

Cole o caminho na barra do Explorador de Ficheiros:

| O quê | Pasta |
|---|---|
| Jogos gravados | `%APPDATA%\com.nextcm.app\saves` |
| Log (para os relatos) | `%LOCALAPPDATA%\com.nextcm.app\logs` |

A versão aparece no canto do menu inicial (por exemplo `0.1.0-beta.1`).

## Como relatar um problema

Abra uma *Issue* no GitHub (ou mande no grupo do beta) com:

1. **Versão** (canto do menu inicial)
2. **O que fez**: os passos até o problema
3. **O que esperava** e **o que aconteceu**
4. Um **print** da tela
5. O ficheiro de **log** (`NextCM.log`, na pasta acima)
6. Se o problema for num jogo gravado, o **save** (`.sqlite` da pasta `saves`), zipado

Também valem opiniões: números estranhos (golos, salários, transferências), textos errados, coisas
diferentes do CM 01/02 original.

## Avisos

- Saves de um beta podem não abrir no beta seguinte. Não se apegue muito à carreira 😉
- Grave com frequência.
