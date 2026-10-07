; Windows installer hooks (Tauri NSIS). ProctorLAN's teacher PC listens for students on the local network,
; so Windows Firewall must allow it. Without this rule Windows asks for administrator approval on first launch.
; The rule is limited to Private and Domain networks and to this program. Failure is harmless: the first-launch
; prompt appears instead (see docs/networking.md).

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="ProctorLAN"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="ProctorLAN" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes profile=private,domain'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="ProctorLAN"'
!macroend
