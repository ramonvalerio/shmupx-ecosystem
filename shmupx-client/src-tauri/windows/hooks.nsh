; Ganchos do instalador NSIS do ShmupX Client (tauri.conf.json › bundle.windows.nsis.installerHooks).

; Firewall do Windows: o ShmupX Emulator recebe os pacotes do parceiro na dupla online. Sem uma
; regra, o Windows mostra o alerta "permitir acesso" na primeira dupla (e a janela do jogo fica por
; cima dele). O instalador (administrador) cria a regra só para o executável do motor, em todas as
; redes. Antes, apaga as regras que já existam para ele: se o jogador fechou ou cancelou o alerta, o
; Windows criou regras de bloqueio, e bloqueio vence permissão.
!define SHMUPX_MOTOR_EXE "$INSTDIR\engine\shmupx-engine.exe"
!macro SHMUPX_FIREWALL_DO_MOTOR
  nsExec::Exec 'netsh advfirewall firewall delete rule name=all program="${SHMUPX_MOTOR_EXE}"'
  Pop $0
  nsExec::Exec 'netsh advfirewall firewall add rule name="ShmupX Emulator" description="Dupla online do ShmupX (treino em dupla)" dir=in action=allow program="${SHMUPX_MOTOR_EXE}" enable=yes profile=any'
  Pop $0
!macroend

; Pastas do jogador ao lado do app (junto de validator, games, ffmpeg e engine):
;   recordings  vídeos das runs gravadas
;   roms        pasta padrão dos ROMs (vazia): o jogador copia os .zip dos jogos para ela
; O Client é instalado para todos os usuários (Program Files) e roda sem ser administrador: o grupo
; Usuários (SID S-1-5-32-545, o mesmo em qualquer idioma do Windows) recebe permissão de gravar nelas.
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$INSTDIR\recordings"
  nsExec::Exec 'icacls "$INSTDIR\recordings" /grant *S-1-5-32-545:(OI)(CI)M /Q'
  Pop $0
  CreateDirectory "$INSTDIR\roms"
  nsExec::Exec 'icacls "$INSTDIR\roms" /grant *S-1-5-32-545:(OI)(CI)M /Q'
  Pop $0
  !insertmacro SHMUPX_FIREWALL_DO_MOTOR
!macroend

; Desinstalar: a regra do motor sai junto.
!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name=all program="${SHMUPX_MOTOR_EXE}"'
  Pop $0
!macroend
