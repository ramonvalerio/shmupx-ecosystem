<#
.SYNOPSIS
  Compila o agente, copia para o MiSTer pela rede e roda o instalador.

.EXAMPLE
  .\scripts\implantar.ps1 -Mister 192.168.0.50
  .\scripts\implantar.ps1 -Mister 192.168.0.50 -Rom /media/fat/games/Genesis/Earthion.md

  Pede a senha do root do MiSTer (padrão: 1) a cada conexão.
#>
param(
    [Parameter(Mandatory)] [string] $Mister,
    [string] $Rom = ''
)
$ErrorActionPreference = 'Stop'
$raiz = Split-Path $PSScriptRoot -Parent
$alvo = 'armv7-unknown-linux-musleabihf'

Push-Location $raiz
try {
    cargo build --release --target $alvo
    if ($LASTEXITCODE -ne 0) { throw 'falha na compilação' }

    # Monta o pacote na mesma estrutura do cartão SD (/media/fat/shmupx).
    $pacote = Join-Path $raiz 'target\pacote-mister\shmupx'
    Remove-Item -Recurse -Force (Split-Path $pacote) -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $pacote | Out-Null
    Copy-Item "target\$alvo\release\shmupx-mister" $pacote
    Copy-Item 'mister\shmupx\*' $pacote -Recurse
    Copy-Item 'perfis' $pacote -Recurse
    # Não sobrescreve perfis já descobertos no MiSTer: eles vão para perfis-novos/ e o
    # instalador só copia os que faltam.
    Rename-Item (Join-Path $pacote 'perfis') 'perfis-novos'

    $destino = "root@${Mister}"
    ssh $destino 'mkdir -p /media/fat/shmupx'
    scp -r "$pacote\*" "${destino}:/media/fat/shmupx/"
    if ($LASTEXITCODE -ne 0) { throw 'falha ao copiar para o MiSTer' }
    $comando = 'cd /media/fat/shmupx && sed -i "s/\r$//" *.sh Scripts/*.sh && mkdir -p perfis && cp -n perfis-novos/*.json perfis/ && ./instalar.sh ' + $Rom
    ssh $destino $comando
    if ($LASTEXITCODE -ne 0) { throw 'o instalador falhou no MiSTer' }
}
finally {
    Pop-Location
}
