<#
.SYNOPSIS
  Abre o ShmupX Client em modo dev (recarrega ao salvar) com a configuração do OpenBao.

.DESCRIPTION
  Por padrão usa os DADOS DE PRODUÇÃO: catálogo, salas, chat e ranking reais, pelo backend no
  Cloud Run. Com -Local, usa os emuladores do Firebase (Docker) e o backend local.

  Toda chave de shmupx/client no OpenBao vira variável de ambiente antes do `tauri dev`:
    VITE_*              lidas pela interface (Vite), ex.: VITE_GOOGLE_DESKTOP_CLIENT_ID
    SHMUPX_BACKEND_URL  embutida no binário Rust na compilação
  Nada é gravado em disco.

.EXAMPLE
  npm run dev:prod      # ou .\scripts\dev.ps1
  npm run dev:local     # ou .\scripts\dev.ps1 -Local
#>
param([switch]$Local)
$ErrorActionPreference = "Stop"
if (-not $env:BAO_ADDR) { $env:BAO_ADDR = "http://127.0.0.1:8200" }

$dados = (bao kv get -format=json -mount=shmupx client | ConvertFrom-Json).data.data
if (-not $dados) { throw "nada em shmupx/client no OpenBao ($env:BAO_ADDR)" }
foreach ($p in $dados.PSObject.Properties) {
    Set-Item -Path "env:$($p.Name)" -Value "$($p.Value)"
}
Write-Host "OpenBao shmupx/client: $(($dados.PSObject.Properties.Name) -join ', ')"

if ($Local) {
    $env:VITE_SHMUPX_ENV = "dev"
    Remove-Item env:SHMUPX_BACKEND_URL -ErrorAction SilentlyContinue
    Write-Host "Modo LOCAL: emuladores do Firebase (9099/8088) e backend em http://localhost:3000" -ForegroundColor Yellow
} else {
    $env:VITE_SHMUPX_ENV = "prd"
    if (-not $env:SHMUPX_BACKEND_URL) {
        # Ainda não está no OpenBao: o endereço público do backend de produção.
        $env:SHMUPX_BACKEND_URL = "https://shmupx-backend-230979440400.us-central1.run.app"
    }
    Write-Host "Modo DEV com DADOS DE PRODUÇÃO: $env:SHMUPX_BACKEND_URL" -ForegroundColor Red
    Write-Host "Tudo o que você fizer (chat, runs, perfil) vale para os jogadores reais." -ForegroundColor Red
}

Set-Location (Split-Path $PSScriptRoot -Parent)
npx tauri dev
