; Installs the bundled VST3 into the standard per-machine VST3 folder and removes it on uninstall.
; Also owns one inbound Windows Firewall allow rule scoped to the installed app binary, replacing
; any earlier inbound rule for that binary (including a block left by a dismissed Windows prompt).
; Each netsh call has a timeout: a stalled firewall service must not leave the install page
; waiting forever. A failed or timed-out rule only means Windows may ask about the firewall later.
!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Installing the TrackMesh VST3 plugin..."
  CreateDirectory "$COMMONFILES64\VST3"
  RMDir /r "$COMMONFILES64\VST3\TrackMesh.vst3"
  CopyFiles /SILENT "$INSTDIR\vst3\TrackMesh.vst3" "$COMMONFILES64\VST3"
  DetailPrint "Removing earlier Windows Firewall rules for TrackMesh..."
  nsExec::Exec /TIMEOUT=30000 '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name=all dir=in program="$INSTDIR\trackmesh-desktop.exe"'
  Pop $0
  DetailPrint "Firewall cleanup result: $0"
  DetailPrint "Allowing TrackMesh in Windows Firewall..."
  nsExec::Exec /TIMEOUT=30000 '"$SYSDIR\netsh.exe" advfirewall firewall add rule name="TrackMesh" dir=in action=allow program="$INSTDIR\trackmesh-desktop.exe" enable=yes profile=any'
  Pop $0
  DetailPrint "Firewall rule result: $0"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$COMMONFILES64\VST3\TrackMesh.vst3"
  DetailPrint "Removing Windows Firewall rules for TrackMesh..."
  nsExec::Exec /TIMEOUT=30000 '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name=all dir=in program="$INSTDIR\trackmesh-desktop.exe"'
  Pop $0
!macroend
