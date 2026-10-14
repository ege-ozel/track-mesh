; Installs the bundled VST3 into the standard per-machine VST3 folder and removes it on uninstall.
!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$COMMONFILES64\VST3"
  RMDir /r "$COMMONFILES64\VST3\TrackMesh.vst3"
  CopyFiles /SILENT "$INSTDIR\vst3\TrackMesh.vst3" "$COMMONFILES64\VST3"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$COMMONFILES64\VST3\TrackMesh.vst3"
!macroend
