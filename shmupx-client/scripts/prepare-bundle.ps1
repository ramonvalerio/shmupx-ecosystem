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
  runtime\    DLLs que o shmupx-client.exe exige e que um Windows recém-instalado não tem: o
              Visual C++ (msvcp140*, vcruntime140*, puxados pelo ONNX Runtime) e o DirectML 1.15
              (o do Windows 10 antigo não tem DMLCreateDevice1). O instalador põe ao lado do exe
              (tauri.bundle.windows.conf.json), então o jogador não instala nada à parte.
  webview2\   instalador oficial do WebView2, só para a versão portátil (make-portable.ps1)

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

# Runtime do Client: sem estas DLLs, um Windows sem o Visual C++ Redistributable mostra
# "MSVCP140_1.dll não foi encontrado" e o app nem abre. A Microsoft permite levá-las ao lado do exe.
New-Item -ItemType Directory -Force "$saida\runtime" | Out-Null
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$crt = Get-ChildItem "$vs\VC\Redist\MSVC" -Directory -ErrorAction SilentlyContinue |
    Where-Object Name -match '^\d+\.\d+\.\d+$' | Sort-Object { [version]$_.Name } -Descending |
    ForEach-Object { Get-ChildItem "$($_.FullName)\x64" -Directory -Filter "Microsoft.VC14*.CRT" } |
    Select-Object -First 1
if (-not $crt) { throw "não achei o Visual C++ redistribuível (VC\Redist\MSVC\*\x64\Microsoft.VC14*.CRT) no Visual Studio" }
foreach ($dll in "msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll") {
    Copy-Item "$($crt.FullName)\$dll" "$saida\runtime\$dll"
}
# O DirectML que o ONNX Runtime (crate ort) baixou junto com ele.
$dml = Get-ChildItem "$env:LOCALAPPDATA\ort.pyke.io\dfbin\x86_64-pc-windows-msvc" -Recurse -Filter DirectML.dll -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $dml) { throw "DirectML.dll do ONNX Runtime não encontrado: compile o Client uma vez (cargo build --release) e rode de novo" }
if ([version]($dml.VersionInfo.FileVersion -replace '[^\d.].*$') -lt [version]"1.15") { throw "DirectML $($dml.VersionInfo.FileVersion) antigo demais em $($dml.FullName)" }
Copy-Item $dml.FullName "$saida\runtime\DirectML.dll"
Write-Host "Runtime: Visual C++ $((Get-Item "$saida\runtime\msvcp140.dll").VersionInfo.ProductVersion), DirectML $($dml.VersionInfo.ProductVersion)"

# Instalador oficial do WebView2 (bootstrapper, ~2 MB): a versão portátil não tem instalador NSIS,
# então o lançador dela roda este quando o Windows não tem o WebView2.
New-Item -ItemType Directory -Force "$saida\webview2" | Out-Null
$webview2 = "$saida\webview2\MicrosoftEdgeWebview2Setup.exe"
Invoke-WebRequest "https://go.microsoft.com/fwlink/p/?LinkId=2124703" -OutFile $webview2 -UseBasicParsing
$assinatura = Get-AuthenticodeSignature $webview2
if ($assinatura.Status -ne "Valid" -or $assinatura.SignerCertificate.Subject -notmatch "O=Microsoft Corporation") {
    throw "instalador do WebView2 sem assinatura válida da Microsoft ($($assinatura.Status))"
}

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
