<#
.SYNOPSIS
  Gera o ShmupX Client para Linux (.deb e AppImage) num contêiner Docker (linux/Dockerfile).

.DESCRIPTION
  Precisa do Docker Desktop rodando. Os pacotes saem em src-tauri\target\linux\. Builds seguintes
  reaproveitam o cache em src-tauri\target\linux-cache\ (venv do motor, target do Rust e a cópia do
  código). Fica no disco do repositório, não no disco virtual do Docker: o target do Rust tem vários
  GB e enchia o C:.
  Variáveis repassadas ao contêiner, se definidas: TAURI_SIGNING_PRIVATE_KEY(_PASSWORD) (assinatura
  do updater) e SHMUPX_BACKEND_URL.
#>
$ErrorActionPreference = "Stop"
$client = Split-Path $PSScriptRoot -Parent
$repo = Split-Path $client -Parent
$saida = Join-Path $client "src-tauri\target\linux"
$cache = Join-Path $client "src-tauri\target\linux-cache"
New-Item -ItemType Directory -Force $saida, "$cache\cache", "$cache\work" | Out-Null

docker build -t shmupx-client-linux (Join-Path $client "linux")
if ($LASTEXITCODE) { throw "falha ao montar a imagem de build" }

$envs = @()
foreach ($nome in "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "SHMUPX_BACKEND_URL") {
    if (Test-Path "env:$nome") { $envs += @("-e", $nome) }
}
docker run --rm @envs `
    -v "${repo}:/src:ro" -v "${saida}:/out" -v "${cache}\cache:/cache" -v "${cache}\work:/work" `
    shmupx-client-linux bash /src/shmupx-client/linux/build.sh
if ($LASTEXITCODE) { throw "o build Linux falhou" }
Get-ChildItem $saida | Select-Object Name, Length
