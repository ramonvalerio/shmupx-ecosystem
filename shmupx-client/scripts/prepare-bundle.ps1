<#
.SYNOPSIS
  Junta em src-tauri\bundle\ o que o instalador leva além do app: motor de validação compilado,
  scripts dos jogos aceitos, FFmpeg e o ShmupX Emulator. O jogador não precisa instalar Python,
  FFmpeg nem emulador.

.DESCRIPTION
  validator\  shmupx-validator.exe + dependências (shmupx-lab\validators\tools\build_validator.ps1)
  games\      só os scripts de src-tauri\scripts-aceitos.json, e só se aprovados no Lab
  ffmpeg\     ffmpeg.exe (de -Ffmpeg ou do PATH) e a licença dele, se encontrada
  engine\     ShmupX Emulator (ShmupX Engine): o pacote do último build do main que passou, da
              release pública windows-latest de ramonvalerio/shmupx-engine (SHA-256 conferido),
              ou o de -Emulador

  Depois: npm run bundle (o tauri build com tauri.bundle.conf.json). O Linux faz o mesmo em linux/build.sh.

.PARAMETER Ffmpeg
  ffmpeg.exe a distribuir. Padrão: o do PATH. Builds com libx264 são GPL: leve a licença junto.
.PARAMETER ReusarMotor
  Não recompila o motor se shmupx-lab\validators\dist\shmupx-validator já existe.
.PARAMETER Emulador
  Pasta de um pacote do ShmupX Engine já baixado (com shmupx-engine.exe), no lugar do build do Actions.
#>
param([string]$Ffmpeg, [switch]$ReusarMotor, [string]$Emulador)
$ErrorActionPreference = "Stop"
$client = Split-Path $PSScriptRoot -Parent
$validators = Resolve-Path (Join-Path $client "..\shmupx-lab\validators")
$saida = Join-Path $client "src-tauri\bundle"

$motor = Join-Path $validators "dist\shmupx-validator"
if (-not ($ReusarMotor -and (Test-Path "$motor\shmupx-validator.exe"))) {
    & (Join-Path $validators "tools\build_validator.ps1")
    if (-not $?) { throw "falha ao compilar o motor" }
}

if (Test-Path $saida) { Remove-Item -Recurse -Force $saida }
New-Item -ItemType Directory -Force "$saida\games", "$saida\ffmpeg" | Out-Null
Copy-Item -Recurse $motor "$saida\validator"

$aceitos = Get-Content -Raw (Join-Path $client "src-tauri\scripts-aceitos.json") | ConvertFrom-Json
foreach ($s in $aceitos) {
    $origem = Join-Path $validators "games\$($s.game)\$($s.version)"
    $manifesto = Get-Content -Raw "$origem\script.json" | ConvertFrom-Json
    if ($manifesto.status -ne "approved") { throw "$($s.game) $($s.version) não está aprovado no Lab" }
    $destino = "$saida\games\$($s.game)\$($s.version)"
    New-Item -ItemType Directory -Force (Split-Path $destino) | Out-Null
    Copy-Item -Recurse $origem $destino
    # Recortes brutos do make_digit_templates.py não fazem parte do script.
    Remove-Item -Recurse -Force "$destino\templates\raw_digits" -ErrorAction SilentlyContinue
}

if (-not $Ffmpeg) { $Ffmpeg = (Get-Command ffmpeg -ErrorAction Stop).Source }
Copy-Item $Ffmpeg "$saida\ffmpeg\ffmpeg.exe"
$licenca = Get-ChildItem (Split-Path (Split-Path $Ffmpeg)) -Filter "LICENSE*" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($licenca) { Copy-Item $licenca.FullName "$saida\ffmpeg\LICENSE.txt" }
else { Write-Warning "licença do FFmpeg não encontrada ao lado de $Ffmpeg; inclua-a antes de distribuir" }

# ShmupX Emulator: sem ele, nenhum jogo de emulador abre no Client instalado.
$urlEmulador = "https://github.com/ramonvalerio/shmupx-engine/releases/download/windows-latest/shmupx-engine-windows.zip"
if ($Emulador) {
    Copy-Item -Recurse $Emulador "$saida\engine"
} else {
    $zip = Join-Path $env:TEMP "shmupx-engine-windows.zip"
    $ProgressPreference = "SilentlyContinue"
    Invoke-WebRequest $urlEmulador -OutFile $zip -UseBasicParsing
    $texto = (Invoke-WebRequest "$urlEmulador.sha256" -UseBasicParsing).Content
    # O PowerShell 5.1 devolve bytes para arquivos binários (application/octet-stream).
    if ($texto -is [byte[]]) { $texto = [Text.Encoding]::ASCII.GetString($texto) }
    $esperado = ($texto.Trim() -split "\s+")[0]
    $hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $esperado.ToLower()) { throw "SHA-256 do ShmupX Engine não confere ($hash, esperado $esperado)" }
    Expand-Archive $zip -DestinationPath "$saida\engine" -Force
    Remove-Item $zip
}
foreach ($item in "shmupx-engine.exe", "plugins\shmupx\init.lua", "COPYING", "VERSION.txt") {
    if (-not (Test-Path "$saida\engine\$item")) { throw "o pacote do ShmupX Engine está incompleto: falta $item" }
}
Get-Content "$saida\engine\VERSION.txt" | Select-Object -First 1

# Prova: o motor empacotado enxerga cada script aceito, completo.
$lista = & "$saida\validator\shmupx-validator.exe" --list-games --games-dir "$saida\games" | ConvertFrom-Json
foreach ($s in $aceitos) {
    $item = $lista | Where-Object { $_.id -eq $s.game -and $_.version -eq $s.version }
    if (-not $item) { throw "o motor empacotado não encontrou $($s.game) $($s.version)" }
    if ($item.missing.Count) { throw "$($s.game) $($s.version) incompleto: $($item.missing -join ', ')" }
}
& "$saida\ffmpeg\ffmpeg.exe" -hide_banner -version | Select-Object -First 1
$mb = [int]((Get-ChildItem -Recurse $saida | Measure-Object Length -Sum).Sum / 1MB)
Write-Host "OK: $saida ($mb MB; $($aceitos.Count) script(s))"
