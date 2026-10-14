; Installs the bundled VST3 into the standard per-machine VST3 folder and removes it on uninstall.
; Also owns one inbound Windows Firewall allow rule scoped to the installed app binary, replacing
; any earlier inbound rule for that binary (including a block left by a dismissed Windows prompt).
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$COMMONFILES64\VST3"
  RMDir /r "$COMMONFILES64\VST3\TrackMesh.vst3"
  CopyFiles /SILENT "$INSTDIR\vst3\TrackMesh.vst3" "$COMMONFILES64\VST3"
  nsExec::Exec '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name=all dir=in program="$INSTDIR\trackmesh-desktop.exe"'
  Pop $0
  nsExec::Exec '"$SYSDIR\netsh.exe" advfirewall firewall add rule name="TrackMesh" dir=in action=allow program="$INSTDIR\trackmesh-desktop.exe" enable=yes profile=any'
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$COMMONFILES64\VST3\TrackMesh.vst3"
  nsExec::Exec '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name=all dir=in program="$INSTDIR\trackmesh-desktop.exe"'
  Pop $0
!macroend
