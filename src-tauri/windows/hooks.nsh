; Nagsworth's additions to Tauri's NSIS installer (bundle > windows > nsis > installerHooks in
; tauri.conf.json). The template itself gives the setup /S (silent), /P (passive), /UPDATE (no
; new shortcuts, the files replaced in place, the Settings > Apps entry kept), /R (start the
; app afterwards, silent and passive only) and /ARGS <its arguments>.
;
; /WAITPID=<pid>: Nagsworth's own update (crates/ns-update, docs/DECISIONS.md D22). The
; Nagsworth that downloaded this setup starts it with
;   /S /UPDATE /R /WAITPID=<its pid> /ARGS --hidden
; and exits; the setup waits up to a minute for that process before the template looks whether
; the app is running (it would otherwise ask Restart Manager to end it), so the exiting
; Nagsworth finishes its last writes, and /R then starts the new version hidden in the tray.
; A process that is gone already, or cannot be opened, is not waited for.

!macro NSIS_HOOK_PREINSTALL
  ${GetOptions} $CMDLINE "/WAITPID=" $R0
  ${IfNot} ${Errors}
    ; SYNCHRONIZE
    System::Call 'kernel32::OpenProcess(i 0x00100000, i 0, i $R0) p .R1'
    ${If} $R1 P<> 0
      DetailPrint "Waiting for Nagsworth (process $R0) to exit"
      System::Call 'kernel32::WaitForSingleObject(p R1, i 60000) i .R2'
      System::Call 'kernel32::CloseHandle(p R1)'
      ${If} $R2 <> 0
        DetailPrint "Nagsworth (process $R0) is still running after a minute"
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend
