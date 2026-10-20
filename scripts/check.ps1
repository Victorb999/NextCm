# Verificação completa do NextCM: formatação e clippy do Rust, testes do core, TypeScript e
# oxlint. Mostra uma linha por passo e, nos que falham, só o fim da saída (os erros).
#
#   powershell -File scripts/check.ps1          # tudo
#   powershell -File scripts/check.ps1 -Quick   # sem os testes do core (~10 s a menos)
#
# Os testes do core também regeneram os tipos TS em game/src/bindings (ts-rs), por isso
# correm antes do tsc.
param([switch]$Quick)

$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$failed = @()

function Step([string]$name, [scriptblock]$cmd) {
    $out = & $cmd 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FALHOU  $name" -ForegroundColor Red
        ($out.Trim() -split "`r?`n" | Select-Object -Last 40) -join "`n" | Write-Host
        $script:failed += $name
    } else {
        Write-Host "ok      $name"
    }
}

Step 'cargo fmt' { cargo fmt --all -- --check }
Step 'cargo clippy' { cargo clippy --workspace --all-targets --quiet -- -D warnings }
if (-not $Quick) { Step 'cargo test (core)' { cargo test -p nextcm-core --release --quiet } }
Step 'tsc' { Push-Location game; npx tsc -b; $code = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $code }
Step 'oxlint' { Push-Location game; npx oxlint --deny-warnings; $code = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $code }

if ($failed.Count -gt 0) {
    Write-Host "`n$($failed.Count) passo(s) falharam: $($failed -join ', ')" -ForegroundColor Red
    exit 1
}
Write-Host "`nTudo ok."
