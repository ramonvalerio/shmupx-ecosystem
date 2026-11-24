<#
.SYNOPSIS
  Compila e abre o ShmupX Client como ele roda instalado no PC do jogador, para testar antes de
  gerar o instalador.

.DESCRIPTION
  O `npm run dev:prod` usa os dados de produção, mas NÃO se comporta como o app instalado:
    - é um build de debug, que roda dentro de um terminal (os programas que ele abre, como o
      ShmupX Engine, usam o mesmo terminal e não abrem janela própria);
    - acha o emulador em shmupx-engine/dist/teste, não na pasta engine/ do instalador;
    - a interface vem do Vite (localhost:1420), não do dist/ compilado.

  Este script faz o que o instalador faz, menos instalar:
    1. lê shmupx/client do OpenBao (dados de produção, como o dev:prod);
    2. junta motor de validação, scripts, FFmpeg e ShmupX Engine em src-tauri\bundle\
       (prepare-bundle.ps1), se ainda não estiverem lá ou com -Preparar;
    3. compila a interface (dist/) e o app em release (sem terminal, sem os atalhos de debug);
    4. põe os recursos (engine/, validator/, games/, ffmpeg/) ao lado do executável, onde o app
       instalado os procura;
    5. abre o app fora do terminal, como um duplo clique.

  O log do app fica em %LOCALAPPDATA%\com.shmupx.client\logs.

.PARAMETER Preparar
  Refaz src-tauri\bundle\ (baixa o último ShmupX Engine do Actions, recompila o motor de validação).
.PARAMETER Emulador
  Pasta de um pacote do ShmupX Engine já baixado, no lugar do build do Actions (implica -Preparar).

.EXAMPLE
  npm run prod
  npm run prod:motor    # o mesmo, baixando antes o último ShmupX Engine (-Preparar)
#>
param([switch]$Preparar, [string]$Emulador)
$ErrorActionPreference = "Stop"
if (-not $env:BAO_ADDR) { $env:BAO_ADDR = "http://127.0.0.1:8200" }

$client = Split-Path $PSScriptRoot -Parent
$tauri = Join-Path $client "src-tauri"
$bundle = Join-Path $tauri "bundle"
# O Client faz parte do workspace do shmupx-ecosystem: o Cargo compila na pasta target da raiz.
$alvo = (cargo metadata --format-version 1 --no-deps --manifest-path (Join-Path $tauri "Cargo.toml") | ConvertFrom-Json).target_directory
if (-not $alvo) { throw "não foi possível descobrir a pasta de build do Cargo" }
$release = Join-Path $alvo "release"
$exe = Join-Path $release "shmupx-client.exe"

# 1. Configuração de produção, igual ao dev:prod.
$dados = (bao kv get -format=json -mount=shmupx client | ConvertFrom-Json).data.data
if (-not $dados) { throw "nada em shmupx/client no OpenBao ($env:BAO_ADDR)" }
foreach ($p in $dados.PSObject.Properties) {
    Set-Item -Path "env:$($p.Name)" -Value "$($p.Value)"
}
$env:VITE_SHMUPX_ENV = "prd"
if (-not $env:SHMUPX_BACKEND_URL) {
    $env:SHMUPX_BACKEND_URL = "https://shmupx-backend-230979440400.us-central1.run.app"
}
Write-Host "Modo PRODUÇÃO (app compilado, como instalado): $env:SHMUPX_BACKEND_URL" -ForegroundColor Red
Write-Host "Tudo o que você fizer (chat, runs, perfil) vale para os jogadores reais." -ForegroundColor Red

# 2. Recursos do instalador.
if ($Emulador) { $Preparar = $true }
if ($Preparar -or -not (Test-Path "$bundle\engine\shmupx-engine.exe")) {
    $opcoes = @{ ReusarMotor = $true }
    if ($Emulador) { $opcoes.Emulador = $Emulador }
    & (Join-Path $PSScriptRoot "prepare-bundle.ps1") @opcoes
    if (-not $?) { throw "falha ao preparar os recursos do instalador" }
}

# Um Client compilado aberto trava o executável: fecha só ele (o instalado fica como está).
Get-Process shmupx-client -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -eq [IO.Path]::GetFullPath($exe)) } |
    ForEach-Object { Write-Host "Fechando o Client compilado aberto (PID $($_.Id))"; Stop-Process -Id $_.Id -Force; $_.WaitForExit(5000) | Out-Null }
Get-Process shmupx-engine -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($release, [StringComparison]::OrdinalIgnoreCase) } |
    ForEach-Object { Stop-Process -Id $_.Id -Force }

# 3. Interface e app em release, com a mesma configuração do instalador.
Set-Location $client
npm run build
if ($LASTEXITCODE) { throw "falha ao compilar a interface" }
npx tauri build --no-bundle --config src-tauri/tauri.bundle.conf.json
if ($LASTEXITCODE) { throw "falha ao compilar o Client" }
if (-not (Test-Path $exe)) { throw "não achei o executável em $exe" }

# 4. No Windows, o app instalado procura os recursos na pasta do próprio executável.
foreach ($pasta in "engine", "validator", "games", "ffmpeg") {
    $destino = Join-Path $release $pasta
    if (Test-Path $destino) { Remove-Item -Recurse -Force $destino }
    Copy-Item -Recurse (Join-Path $bundle $pasta) $destino
}

# 5. Abre fora do terminal.
Write-Host "Abrindo $exe"
Write-Host "Log: $env:LOCALAPPDATA\com.shmupx.client\logs"
Start-Process -FilePath $exe -WorkingDirectory $release
