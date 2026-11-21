<#
.SYNOPSIS
  Gera o ShmupX Client para Linux (.deb e AppImage) num contêiner Docker (linux/Dockerfile).

.DESCRIPTION
  Precisa do Docker Desktop rodando. Os pacotes saem em src-tauri\target\linux\. Builds seguintes
  reaproveitam o cache em src-tauri\target\linux-cache\ (venv do motor, target do Rust e a cópia do
  código). Fica no disco do repositório, não no disco virtual do Docker: o target do Rust tem vários
  GB e enchia o C:.
  Variáveis repassadas ao contêiner, se definidas: TAURI_SIGNING_PRIVATE_KEY(_PASSWORD) (assinatura
  do updater) e SHMUPX_BACKEND_URL. As chaves de shmupx/client no OpenBao (VITE_GOOGLE_DESKTOP_CLIENT_ID)
  são lidas e repassadas também.
#>
$ErrorActionPreference = "Stop"
if (-not $env:BAO_ADDR) { $env:BAO_ADDR = "http://127.0.0.1:8200" }
$dados = (bao kv get -format=json -mount=shmupx client | ConvertFrom-Json).data.data
if (-not $dados) { throw "nada em shmupx/client no OpenBao ($env:BAO_ADDR)" }
foreach ($p in $dados.PSObject.Properties) { Set-Item -Path "env:$($p.Name)" -Value "$($p.Value)" }
$client = Split-Path $PSScriptRoot -Parent
$repo = Split-Path $client -Parent
$saida = Join-Path $client "src-tauri\target\linux"
$cache = Join-Path $client "src-tauri\target\linux-cache"
New-Item -ItemType Directory -Force $saida, "$cache\cache", "$cache\work" | Out-Null

docker build -t shmupx-client-linux (Join-Path $client "linux")
if ($LASTEXITCODE) { throw "falha ao montar a imagem de build" }

$envs = @()
foreach ($nome in "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "SHMUPX_BACKEND_URL", "VITE_GOOGLE_DESKTOP_CLIENT_ID") {
    if (Test-Path "env:$nome") { $envs += @("-e", $nome) }
}
docker run --rm @envs `
    -v "${repo}:/src:ro" -v "${saida}:/out" -v "${cache}\cache:/cache" -v "${cache}\work:/work" `
    shmupx-client-linux bash /src/shmupx-client/linux/build.sh
if ($LASTEXITCODE) { throw "o build Linux falhou" }
Get-ChildItem $saida | Select-Object Name, Length
