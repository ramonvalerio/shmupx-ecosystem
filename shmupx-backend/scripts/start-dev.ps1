# Inicia o shmupx-backend em desenvolvimento.
#
# As variáveis (credenciais e secrets) vêm de um arquivo FORA do repositório, uma por linha no
# formato NOME=valor (linhas vazias e iniciadas por # são ignoradas). O modelo está em
# `dev.env.example`. Por padrão o arquivo é %USERPROFILE%\.secrets\shmupx-backend.env.
param(
    [string]$EnvFile = (Join-Path $env:USERPROFILE ".secrets\shmupx-backend.env")
)

$ErrorActionPreference = "Stop"
$backend = Split-Path -Parent $PSScriptRoot

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

# O Windows não deixa o Cargo substituir o .exe enquanto o backend antigo ainda roda.
Stop-Process -Name shmupx-backend -Force -ErrorAction SilentlyContinue

Set-Location $backend
cargo run
