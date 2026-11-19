# Inicia o shmupx-backend localmente.
#
# Ambientes:
#   dev (padrão): Firestore e login nos emuladores do Firebase em Docker (shmupx-infra/firebase,
#                 projeto demo-shmupx). Nada toca produção; sem conta de serviço nem e-mail real.
#   prd         : o projeto real (shmupx-project-id, banco "(default)"). Só com -Ambiente prd, de
#                 propósito: um teste aqui mexe nos dados e avisa os jogadores de verdade.
#
# As variáveis (secrets do OAuth, ADMIN_EMAILS, credenciais de produção) vêm de um arquivo FORA do
# repositório, uma por linha no formato NOME=valor (linhas vazias e iniciadas por # são ignoradas).
# O modelo está em `dev.env.example`. Por padrão o arquivo é %USERPROFILE%\.secrets\shmupx-backend.env.
param(
    [ValidateSet("dev", "prd")]
    [string]$Ambiente = "dev",
    [string]$EnvFile = (Join-Path $env:USERPROFILE ".secrets\shmupx-backend.env")
)

$ErrorActionPreference = "Stop"
$backend = Split-Path -Parent $PSScriptRoot
$firebase = Join-Path (Split-Path -Parent $backend) "shmupx-infra\firebase"

if (-not (Test-Path -LiteralPath $EnvFile)) {
    Write-Host "Arquivo de variáveis não encontrado: $EnvFile" -ForegroundColor Red
    Write-Host "Copie $backend\dev.env.example para esse caminho, preencha os valores e rode de novo."
    Write-Host "Para usar outro caminho: start-dev.ps1 -EnvFile 'C:\caminho\arquivo.env'"
    exit 1
}

foreach ($linha in Get-Content -LiteralPath $EnvFile) {
    $linha = $linha.Trim()
    if ($linha -eq "" -or $linha.StartsWith("#")) { continue }
    $posicao = $linha.IndexOf("=")
    if ($posicao -lt 1) { continue }
    $nome = $linha.Substring(0, $posicao).Trim()
    $valor = $linha.Substring($posicao + 1).Trim().Trim('"')
    # Um caminho sem pasta (ex.: só o nome do JSON) é procurado ao lado do arquivo de variáveis.
    if ($nome -eq "GOOGLE_APPLICATION_CREDENTIALS" -and $valor -ne "" -and -not [System.IO.Path]::IsPathRooted($valor)) {
        $valor = Join-Path (Split-Path -Parent (Resolve-Path -LiteralPath $EnvFile)) $valor
    }
    [Environment]::SetEnvironmentVariable($nome, $valor, "Process")
}

if ($Ambiente -eq "dev") {
    # Produção fica inalcançável: sem conta de serviço, sem e-mail real, projeto demo-*.
    foreach ($nome in "GOOGLE_APPLICATION_CREDENTIALS", "AWS_SECRETS_ROLE_ARN", "AWS_SECRET_ID", "EMAIL_FROM") {
        [Environment]::SetEnvironmentVariable($nome, $null, "Process")
    }
    $env:FIREBASE_PROJECT_ID = "demo-shmupx"
    $env:FIRESTORE_EMULATOR_HOST = "127.0.0.1:8088"
    $env:FIREBASE_AUTH_EMULATOR_HOST = "127.0.0.1:9099"

    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Write-Host "Docker não encontrado. Instale o Docker Desktop para o ambiente de desenvolvimento." -ForegroundColor Red
        exit 1
    }
    docker info *> $null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "O Docker Desktop não está rodando. Abra-o e rode este script de novo." -ForegroundColor Red
        exit 1
    }
    Write-Host "Subindo os emuladores do Firebase (Docker)..." -ForegroundColor Cyan
    docker compose -f (Join-Path $firebase "docker-compose.yml") up -d --build
    if ($LASTEXITCODE -ne 0) { exit 1 }
    # O Firestore do emulador leva alguns segundos para aceitar conexões.
    $pronto = $false
    foreach ($tentativa in 1..60) {
        try { Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 "http://127.0.0.1:8088/" *> $null; $pronto = $true; break } catch { Start-Sleep -Seconds 1 }
    }
    if (-not $pronto) {
        Write-Host "O emulador do Firestore não respondeu em 60 s. Veja: docker logs shmupx-firebase-emulators" -ForegroundColor Red
        exit 1
    }
    Write-Host "DESENVOLVIMENTO: emuladores em http://127.0.0.1:4000 (dados em shmupx-infra\firebase\emulator-data)." -ForegroundColor Green
} else {
    foreach ($nome in "FIRESTORE_EMULATOR_HOST", "FIREBASE_AUTH_EMULATOR_HOST") {
        [Environment]::SetEnvironmentVariable($nome, $null, "Process")
    }
    Write-Host "ATENÇÃO: backend local ligado à PRODUÇÃO ($env:FIREBASE_PROJECT_ID). Dados e avisos são reais." -ForegroundColor Yellow
}

# O Windows não deixa o Cargo substituir o .exe enquanto o backend antigo ainda roda.
Stop-Process -Name shmupx-backend -Force -ErrorAction SilentlyContinue

Set-Location $backend
cargo run
