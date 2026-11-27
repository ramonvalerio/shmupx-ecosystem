<#
.SYNOPSIS
  Publica a interface do ShmupX Client sem instalador (src-tauri/src/interface.rs).

.DESCRIPTION
  Gera a interface de produção (npm run build), monta o manifesto com o SHA-256 de cada arquivo,
  assina com a chave do updater (OpenBao shmupx/updater, a mesma do instalador) e envia ao S3:
    downloads/ui/files/<sha256>             arquivos (só os novos sobem; nomes pelo conteúdo)
    downloads/ui/<versão>/manifest.json     manifesto e a assinatura (.sig), sem cache
  Os Clients instalados nessa versão exata baixam o que falta e recarregam a interface sozinhos.

  Só serve para mudanças de interface. Se o programa (src-tauri) mudou desde o commit
  "ShmupX Client <versão>", a interface pode chamar um comando que os Clients instalados não têm: o
  script recusa (use -Forcar só quando a mudança no src-tauri não afeta a interface, ex.: mapas do
  motor). Rode no PowerShell 7 com o OpenBao local destravado.

.PARAMETER Forcar
  Publica mesmo com mudanças no src-tauri desde a versão ou arquivos não commitados.

.PARAMETER SemEnviar
  Só gera e assina (pasta de saída no fim), sem enviar ao S3.
#>
param([switch]$Forcar, [switch]$SemEnviar)
$ErrorActionPreference = "Stop"
$client = Split-Path $PSScriptRoot -Parent
$repo = Split-Path $client -Parent
$bucket = "shmupx-frontend-prod-364037626379"
$distribuicao = "E30ZYCLQBY4WFS"

$versao = (Get-Content -Raw "$client\src-tauri\tauri.conf.json" | ConvertFrom-Json).version
$release = git -C $repo log -1 --format=%H --grep="^ShmupX Client $([regex]::Escape($versao))$"
if (-not $release) { throw "Não achei o commit 'ShmupX Client $versao' (a versão instalada pelos jogadores)." }
$mudouPrograma = @(git -C $repo diff --name-only $release HEAD -- shmupx-client/src-tauri)
$naoCommitado = @(git -C $repo status --porcelain -- shmupx-client/src shmupx-client/public shmupx-client/src-tauri shmupx-client/index.html shmupx-client/package.json)
if (($mudouPrograma.Count -gt 0 -or $naoCommitado.Count -gt 0) -and -not $Forcar) {
    if ($mudouPrograma.Count -gt 0) { Write-Host "O programa mudou desde a $versao (precisa de instalador):"; $mudouPrograma | ForEach-Object { Write-Host "  $_" } }
    if ($naoCommitado.Count -gt 0) { Write-Host "Há mudanças não commitadas:"; $naoCommitado | ForEach-Object { Write-Host "  $_" } }
    throw "Interface não publicada (use -Forcar se tiver certeza)."
}

# Ambiente de produção, como no "Gerar instalador" do Lab: shmupx/client e a chave do updater.
if (-not $env:BAO_ADDR) { $env:BAO_ADDR = "http://127.0.0.1:8200" }
foreach ($caminho in "client", "updater") {
    $dados = (bao kv get -format=json -mount=shmupx $caminho | ConvertFrom-Json).data.data
    if (-not $dados) { throw "nada em shmupx/$caminho no OpenBao" }
    foreach ($p in $dados.PSObject.Properties) { Set-Item -Path "env:$($p.Name)" -Value "$($p.Value)" }
}
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) { throw "sem TAURI_SIGNING_PRIVATE_KEY" }
$env:VITE_SHMUPX_ENV = "prd"
$built = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$env:SHMUPX_UI_BUILT = $built

Push-Location $client
try {
    npm run build
    if ($LASTEXITCODE) { throw "falha no build da interface" }
} finally {
    Pop-Location
}

$dist = Join-Path $client "dist"
$saida = Join-Path ([IO.Path]::GetTempPath()) "shmupx-interface-$($built -replace '[:]', '')"
Remove-Item -Recurse -Force $saida -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$saida\files" | Out-Null

# manifesto: caminho como o Tauri pede (/assets/x.js) -> sha256 e tamanho
$arquivos = [ordered]@{}
foreach ($f in Get-ChildItem -Recurse -File $dist | Sort-Object FullName) {
    $chave = "/" + $f.FullName.Substring($dist.Length + 1).Replace("\", "/")
    $sha = (Get-FileHash -Algorithm SHA256 $f.FullName).Hash.ToLowerInvariant()
    $arquivos[$chave] = [ordered]@{ sha256 = $sha; size = $f.Length }
    Copy-Item $f.FullName "$saida\files\$sha" -Force
}
$commit = (git -C $repo rev-parse --short HEAD).Trim()
$manifesto = [ordered]@{ native = $versao; built = $built; commit = $commit; files = $arquivos }
$caminhoManifesto = Join-Path $saida "manifest.json"
# sem BOM: a assinatura cobre os bytes exatos
[IO.File]::WriteAllText($caminhoManifesto, ($manifesto | ConvertTo-Json -Depth 5 -Compress), [Text.UTF8Encoding]::new($false))

Push-Location $client
try {
    npx tauri signer sign $caminhoManifesto
    if ($LASTEXITCODE) { throw "falha ao assinar o manifesto" }
} finally {
    Pop-Location
}
if (-not (Test-Path "$caminhoManifesto.sig")) { throw "a assinatura não foi gerada" }
Write-Host "Interface $built (commit $commit) para a versão ${versao}: $($arquivos.Count) arquivos em $saida"
if ($SemEnviar) { return }

$aws = (bao kv get -format=json -mount=shmupx infra/aws | ConvertFrom-Json).data.data
$env:AWS_ACCESS_KEY_ID = $aws.AWS_ACCESS_KEY_ID
$env:AWS_SECRET_ACCESS_KEY = $aws.AWS_SECRET_ACCESS_KEY
$env:AWS_DEFAULT_REGION = "us-east-1"
# arquivos pelo conteúdo: os que já estão lá (mesmo nome = mesmo conteúdo) não sobem de novo
aws s3 sync "$saida\files" "s3://$bucket/downloads/ui/files/" --size-only --content-type "application/octet-stream" --cache-control "public,max-age=31536000,immutable" --only-show-errors
if ($LASTEXITCODE) { throw "falha ao enviar os arquivos" }
aws s3 cp "$caminhoManifesto.sig" "s3://$bucket/downloads/ui/$versao/manifest.json.sig" --content-type "text/plain" --cache-control "no-cache" --only-show-errors
if ($LASTEXITCODE) { throw "falha ao enviar a assinatura" }
aws s3 cp $caminhoManifesto "s3://$bucket/downloads/ui/$versao/manifest.json" --content-type "application/json" --cache-control "no-cache" --only-show-errors
if ($LASTEXITCODE) { throw "falha ao enviar o manifesto" }
aws cloudfront create-invalidation --distribution-id $distribuicao --paths "/downloads/ui/$versao/*" --query "Invalidation.Id" --output text
Write-Host "Publicada: os Clients ${versao} recebem a interface $built nos próximos minutos."
