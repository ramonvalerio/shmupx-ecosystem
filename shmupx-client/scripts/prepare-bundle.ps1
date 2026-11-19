<#
.SYNOPSIS
  Junta em src-tauri\bundle\ o que o instalador leva além do app: motor de validação compilado,
  scripts dos jogos aceitos e FFmpeg. O jogador não precisa instalar Python nem FFmpeg.

.DESCRIPTION
  validator\  shmupx-validator.exe + dependências (shmupx-lab\validators\tools\build_validator.ps1)
  games\      só os scripts de src-tauri\scripts-aceitos.json, e só se aprovados no Lab
  ffmpeg\     ffmpeg.exe (de -Ffmpeg ou do PATH) e a licença dele, se encontrada

  Depois: npm run tauri build -- --config src-tauri/tauri.bundle.conf.json (ou npm run bundle).

.PARAMETER Ffmpeg
  ffmpeg.exe a distribuir. Padrão: o do PATH. Builds com libx264 são GPL: leve a licença junto.
.PARAMETER ReusarMotor
  Não recompila o motor se shmupx-lab\validators\dist\shmupx-validator já existe.
#>
param([string]$Ffmpeg, [switch]$ReusarMotor)
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
