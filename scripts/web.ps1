# Abre o NextCM no navegador: compila a ponte de dev (o core, em release para o Continuar
# Jogo ser rápido), liga-a na porta 1421 e corre o Vite (a interface) na 5173, que abre o
# navegador. Ctrl+C fecha os dois.
#
#   powershell -File scripts/web.ps1             # compila e abre
#   powershell -File scripts/web.ps1 -NoBuild    # usa a ponte já compilada
#   powershell -File scripts/web.ps1 -NoOpen     # não abre o navegador sozinho
#
# O jogo aberto vive na memória da ponte: fechá-la perde o que não foi gravado.
param([switch]$NoBuild, [switch]$NoOpen)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$bridge = Join-Path $root 'target\release\examples\dev_bridge.exe'

function PortInUse([int]$port) {
    [bool](Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue)
}

if (PortInUse 1421) {
    Write-Host 'A porta 1421 já está ocupada (outra ponte de dev aberta?). Feche-a e tente de novo.' -ForegroundColor Red
    exit 1
}

if (-not $NoBuild) {
    Write-Host 'A compilar a ponte de dev (release)...'
    cargo build -p nextcm-core --release --example dev_bridge
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (-not (Test-Path $bridge)) {
    Write-Host "Não encontrei $bridge. Corra sem -NoBuild." -ForegroundColor Red
    exit 1
}

if (-not (Test-Path (Join-Path $root 'game\node_modules'))) {
    Write-Host 'A instalar as dependências do front (npm install)...'
    npm --prefix game install
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$proc = Start-Process -FilePath $bridge -WorkingDirectory $root -NoNewWindow -PassThru
try {
    # Espera a ponte abrir a porta (lê o banco ao arrancar)
    for ($i = 0; $i -lt 60 -and -not (PortInUse 1421); $i++) {
        if ($proc.HasExited) { throw 'A ponte de dev fechou ao arrancar.' }
        Start-Sleep -Milliseconds 500
    }
    Write-Host 'Ponte de dev na porta 1421. A abrir o NextCM em http://localhost:5173 ...' -ForegroundColor Green
    if ($NoOpen) { npm --prefix game run dev } else { npm --prefix game run dev -- --open }
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    Write-Host 'Ponte de dev fechada.'
}
