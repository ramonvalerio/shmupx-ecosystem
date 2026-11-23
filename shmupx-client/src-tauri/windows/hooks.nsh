; Ganchos do instalador NSIS do ShmupX Client (tauri.conf.json › bundle.windows.nsis.installerHooks).

; Pasta das gravações das runs ao lado do app (junto de validator, games e ffmpeg). O Client é
; instalado para todos os usuários (Program Files) e roda sem ser administrador: o grupo Usuários
; (SID S-1-5-32-545, o mesmo em qualquer idioma do Windows) recebe permissão de gravar nela.
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$INSTDIR\recordings"
  nsExec::Exec 'icacls "$INSTDIR\recordings" /grant *S-1-5-32-545:(OI)(CI)M /Q'
  Pop $0
!macroend
