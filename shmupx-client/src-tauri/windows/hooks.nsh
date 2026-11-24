; Ganchos do instalador NSIS do ShmupX Client (tauri.conf.json › bundle.windows.nsis.installerHooks).

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
!macroend
