<#
.SYNOPSIS
  Gera o ShmupX Portable: um único .exe que o jogador abre com dois cliques, sem instalar nada.

.DESCRIPTION
  Roda depois do tauri build (o `npm run bundle` já chama). Junta numa pasta o que o instalador
  põe em Arquivos de Programas — shmupx-client.exe, runtime (Visual C++ e DirectML), engine,
  validator, games, ffmpeg — mais o instalador do WebView2 e a marca `.portable`, e anexa tudo,
  compactado, ao lançador (shmupx-client\portable).

  Saída, ao lado do instalador NSIS:
    target\release\bundle\portable\ShmupX-Portable-<versão>.exe
    target\release\bundle\portable\ShmupX-Portable-<versão>.exe.sha256

  O lançador é compilado com o runtime do C embutido (+crt-static): ele próprio também tem de abrir
  num Windows sem o Visual C++ Redistributable.
#>
$ErrorActionPreference = "Stop"
$client = Split-Path $PSScriptRoot -Parent
$tauri = Join-Path $client "src-tauri"
$bundle = Join-Path $tauri "bundle"
$raiz = Split-Path $client -Parent
$release = Join-Path $raiz "target\release"
$versao = (Get-Content -Raw (Join-Path $tauri "tauri.conf.json") | ConvertFrom-Json).version
$exe = Join-Path $release "shmupx-client.exe"
if (-not (Test-Path $exe)) { throw "não achei ${exe}: rode o tauri build antes" }
foreach ($item in "runtime\DirectML.dll", "webview2\MicrosoftEdgeWebview2Setup.exe", "engine\shmupx-engine.exe", "validator\shmupx-validator.exe") {
    if (-not (Test-Path (Join-Path $bundle $item))) { throw "falta $item em ${bundle}: rode scripts\prepare-bundle.ps1" }
}

# 1. Lançador e empacotador (pasta de build própria: as flags do CRT estático não invalidam o build do Client).
$env:RUSTFLAGS = "-C target-feature=+crt-static"
cargo build --release -p shmupx-portable --target-dir (Join-Path $raiz "target\portable") --manifest-path (Join-Path $raiz "Cargo.toml")
if ($LASTEXITCODE) { throw "falha ao compilar o lançador do ShmupX Portable" }
Remove-Item Env:RUSTFLAGS
$ferramentas = Join-Path $raiz "target\portable\release"

# 2. A pasta do Client como o jogador vai tê-la.
$saida = Join-Path $release "bundle\portable"
$pasta = Join-Path $saida "ShmupX"
if (Test-Path $pasta) { Remove-Item -Recurse -Force $pasta }
New-Item -ItemType Directory -Force $pasta | Out-Null
Copy-Item $exe $pasta
Copy-Item (Join-Path $bundle "runtime\*.dll") $pasta
foreach ($item in "engine", "validator", "games", "ffmpeg", "webview2") {
    Copy-Item -Recurse (Join-Path $bundle $item) (Join-Path $pasta $item)
}
# O Client lê esta marca: ROMs e gravações ficam fora da pasta da versão, e a atualização baixa o
# próximo ShmupX Portable em vez do instalador.
Set-Content -Path (Join-Path $pasta ".portable") -Value "ShmupX Portable $versao" -Encoding ascii

# 3. Lançador + pacote num .exe só.
Get-ChildItem $saida -Filter "ShmupX-Portable-*.exe*" | Remove-Item -Force
$final = Join-Path $saida "ShmupX-Portable-$versao.exe"
& (Join-Path $ferramentas "shmupx-portable-pack.exe") $pasta (Join-Path $ferramentas "shmupx-portable.exe") $versao $final
if ($LASTEXITCODE) { throw "falha ao empacotar o ShmupX Portable" }
Remove-Item -Recurse -Force $pasta
# SHA-256 pelo .NET: o Get-FileHash às vezes não carrega no Windows PowerShell aberto pelo pwsh 7.
$leitor = [IO.File]::OpenRead($final)
try { $hash = -join ([Security.Cryptography.SHA256]::Create().ComputeHash($leitor) | ForEach-Object { $_.ToString("x2") }) }
finally { $leitor.Dispose() }
Set-Content -Path "$final.sha256" -Value $hash -Encoding ascii
Write-Host "ShmupX Portable: $final (SHA-256 $hash)"
