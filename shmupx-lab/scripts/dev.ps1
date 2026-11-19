<#
.SYNOPSIS
  Abre o Lab em modo dev com a configuração do OpenBao (mount `shmupx`, caminho `lab`).

.DESCRIPTION
  Toda chave de shmupx/lab vira variável de ambiente antes do `tauri dev`:
    VITE_*              lidas pela interface (Vite), ex.: VITE_BACKEND_URL, VITE_SHMUPX_ENV
    SHMUPX_BACKEND_URL  embutida no binário Rust na compilação
  Nada é gravado em disco; .env.local não é necessário.

.EXAMPLE
  .\scripts\dev.ps1
#>
param([string]$Caminho = "lab")
$ErrorActionPreference = "Stop"
if (-not $env:BAO_ADDR) { $env:BAO_ADDR = "http://127.0.0.1:8200" }

$dados = (bao kv get -format=json -mount=shmupx $Caminho | ConvertFrom-Json).data.data
if (-not $dados) { throw "nada em shmupx/$Caminho no OpenBao ($env:BAO_ADDR)" }
foreach ($p in $dados.PSObject.Properties) {
    Set-Item -Path "env:$($p.Name)" -Value "$($p.Value)"
}
Write-Host "OpenBao shmupx/${Caminho}: $(($dados.PSObject.Properties.Name) -join ', ')"
if (-not $env:VITE_BACKEND_URL) { Write-Warning "VITE_BACKEND_URL ausente em shmupx/$Caminho: a interface vai usar http://localhost:3000" }

Set-Location (Split-Path $PSScriptRoot -Parent)
npx tauri dev
