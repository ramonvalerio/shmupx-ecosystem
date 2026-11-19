<#
.SYNOPSIS
  Compila o motor (engine/) em dist\shmupx-validator\shmupx-validator.exe, sem exigir Python de quem usa.

.DESCRIPTION
  Usa um ambiente isolado (.venv-build) com as versões de requirements-build.txt. Os scripts dos
  jogos (games/) NÃO entram no binário: o Client os empacota ao lado e passa --games-dir.
  O auditor externo (engine.grok) fica de fora: o Client nunca chama APIs externas.

.EXAMPLE
  .\tools\build_validator.ps1
#>
param([string]$Python = "py -3.10")
$ErrorActionPreference = "Stop"
$raiz = Split-Path $PSScriptRoot -Parent
Push-Location $raiz
try {
    $venv = Join-Path $raiz ".venv-build"
    if (-not (Test-Path "$venv\Scripts\python.exe")) {
        Invoke-Expression "$Python -m venv `"$venv`""
        if ($LASTEXITCODE) { throw "falha ao criar $venv" }
    }
    $py = "$venv\Scripts\python.exe"
    & $py -m pip install --disable-pip-version-check -q -r requirements-build.txt
    if ($LASTEXITCODE) { throw "falha ao instalar requirements-build.txt" }

    & $py -m PyInstaller analyze.py --noconfirm --clean --onedir --console `
        --name shmupx-validator `
        --distpath dist --workpath build `
        --hidden-import engine.analyzers.hud_score `
        --exclude-module engine.grok --exclude-module tkinter
    if ($LASTEXITCODE) { throw "PyInstaller falhou" }

    # Prova mínima: o .exe lista os scripts lendo games/ de fora do binário.
    $exe = Join-Path $raiz "dist\shmupx-validator\shmupx-validator.exe"
    $lista = & $exe --list-games --games-dir (Join-Path $raiz "games") | ConvertFrom-Json
    if ($LASTEXITCODE -or -not $lista) { throw "o .exe compilado não listou os scripts" }
    Write-Host "OK: $exe ($($lista.Count) scripts visíveis)"
} finally {
    Pop-Location
}
