; -*- coding: utf-8 -*-
; EN-US: Per-user, versioned installs; generated inventories never recursively delete user data.
; PT-BR: Instalações versionadas por usuário; inventários gerados nunca apagam dados pessoais recursivamente.
Unicode true
RequestExecutionLevel user
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "WordFunc.nsh"
!include "x64.nsh"
!include "config.nsh"
!include "payload.nsh"
; EN-US: Sign the embedded uninstaller before NSIS compresses it into the installer.
; PT-BR: Assina o desinstalador embutido antes de o NSIS compactá-lo no instalador.
!ifdef PAPO_UNINSTALLER_SIGN_COMMAND
!uninstfinalize '${PAPO_UNINSTALLER_SIGN_COMMAND}' = 0
!endif
Name "${PAPO_INSTALLER_LABEL}"
OutFile "${PAPO_OUTPUT}"
SetCompressor /SOLID lzma
CRCCheck force
ManifestDPIAware true
VIProductVersion "${PAPO_VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "Papo"
VIAddVersionKey /LANG=1033 "FileDescription" "Papo per-user preview installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${PAPO_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Papo Contributors; GPL-3.0-only"
!define MUI_ICON "${PAPO_ICON}"
!define MUI_UNICON "${PAPO_ICON}"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PAPO_LICENSE}"
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\Papo.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_FINISH
!define MUI_UNCONFIRMPAGE_TEXT_TOP "Program files will be removed. Settings, caches and saved sessions will be kept. / Apenas os arquivos do programa serão removidos. Configurações, caches e sessões salvas serão preservados."
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "PortugueseBR"
LangString CloseApp ${LANG_ENGLISH} "Close Papo before installing or uninstalling, then retry."
LangString CloseApp ${LANG_PORTUGUESEBR} "Feche o Papo antes de instalar ou desinstalar e tente novamente."
LangString Failed ${LANG_ENGLISH} "Setup could not finish safely. Close Papo and retry this installer. Existing settings and saved sessions were kept."
LangString Failed ${LANG_PORTUGUESEBR} "A instalação não pôde ser concluída com segurança. Feche o Papo e tente novamente. Configurações e sessões salvas foram preservadas."
LangString Owned ${LANG_ENGLISH} "Another installation owns this location or registration. No files were removed."
LangString Owned ${LANG_PORTUGUESEBR} "Outra instalação é proprietária deste local ou registro. Nenhum arquivo foi removido."
LangString Architecture ${LANG_ENGLISH} "This preview requires x64 Windows."
LangString Architecture ${LANG_PORTUGUESEBR} "Esta prévia requer Windows x64."
LangString Downgrade ${LANG_ENGLISH} "A newer Papo version is installed. Uninstall it explicitly before installing an older version."
LangString Downgrade ${LANG_PORTUGUESEBR} "Uma versão mais recente do Papo está instalada. Desinstale-a explicitamente antes de instalar uma versão anterior."
!define STATE "Software\Papo\Installer"
!define ARP "Software\Microsoft\Windows\CurrentVersion\Uninstall\PapoGtk"
Var Root
Var Slot
Var Previous
Var PreviousDesktop
Var Pending
Var Result
Var Failure
Var Params
Var CurrentOwner
Var SetupMutex
Var Cursor
Var Found

; EN-US: Shared recovery code is compiled into every installer and uninstaller.
; PT-BR: O código de recuperação compartilhado é compilado em cada instalador e desinstalador.
!macro Common Prefix
Function ${Prefix}Initialize
    SetShellVarContext current
    SetRegView 64
    StrCpy $Root "$LOCALAPPDATA\Programs\Papo"
    ${IfNot} ${RunningX64}
        MessageBox MB_OK|MB_ICONSTOP "$(Architecture)" /SD IDOK
        SetErrorLevel 1
        Quit
    ${EndIf}
!if "${Prefix}" == "un."
    ; EN-US: The parent keeps the setup mutex while an inactive slot removes only its own files.
    ; PT-BR: O pai mantém o mutex enquanto uma versão inativa remove somente seus próprios arquivos.
    ${GetParameters} $Params
    ${GetOptions} "$Params" "/RETIRED" $0
    ${IfNot} ${Errors}
        ReadRegStr $0 HKCU "${STATE}" "CurrentPath"
        ReadRegStr $1 HKCU "${STATE}" "PendingPath"
        ${If} $0 != $INSTDIR
        ${AndIf} $1 == ""
            Goto ${Prefix}mutex_ready
        ${EndIf}
    ${EndIf}
!endif
    System::Call 'kernel32::CreateMutexW(p 0, i 0, w "Local\${PAPO_APP_ID}.setup") p.s ?e'
    Pop $0
    Pop $SetupMutex
    ${If} $0 = 183
    ${OrIf} $SetupMutex = 0
        SetErrorLevel 1
        Quit
    ${EndIf}
!if "${Prefix}" == "un."
${Prefix}mutex_ready:
!endif
    ReadRegStr $0 HKCU "${STATE}" "Owner"
    ${If} $0 != ""
    ${AndIf} $0 != "${PAPO_APP_ID}"
        Goto ${Prefix}ownership_failure
    ${EndIf}
    ReadRegStr $0 HKCU "${ARP}" "PapoOwner"
    ${If} $0 != ""
    ${AndIf} $0 != "${PAPO_APP_ID}"
        Goto ${Prefix}ownership_failure
    ${EndIf}
    ReadRegStr $1 HKCU "${ARP}" "DisplayName"
    ${If} $1 != ""
    ${AndIf} $0 != "${PAPO_APP_ID}"
        Goto ${Prefix}ownership_failure
    ${EndIf}
    ReadRegStr $0 HKCU "${STATE}" "Owner"
    ReadRegStr $1 HKCU "${STATE}" "CurrentPath"
    ${If} $1 != ""
    ${AndIf} $0 != "${PAPO_APP_ID}"
        Goto ${Prefix}ownership_failure
    ${EndIf}
    ; EN-US: Refuse reparse directories instead of following them during cleanup.
    ; PT-BR: Recusa diretórios de redirecionamento em vez de segui-los durante a limpeza.
    StrCpy $Slot "$Root"
    Call ${Prefix}NoReparse
    StrCpy $Slot "$Root\versions"
    Call ${Prefix}NoReparse
    Return
${Prefix}ownership_failure:
    MessageBox MB_OK|MB_ICONSTOP "$(Owned)" /SD IDOK
    SetErrorLevel 1
    Quit
FunctionEnd

Function ${Prefix}NoReparse
    System::Call 'kernel32::GetFileAttributesW(w "$Slot") i.r0'
    ${If} $0 != -1
        IntOp $0 $0 & 0x400
        ${If} $0 != 0
            Goto ${Prefix}invalid_slot
        ${EndIf}
    ${EndIf}
    Return
${Prefix}invalid_slot:
    MessageBox MB_OK|MB_ICONSTOP "$(Owned)" /SD IDOK
    SetErrorLevel 1
    Quit
FunctionEnd

Function ${Prefix}AssertSlot
    ${GetParent} "$Slot" $0
    ${If} $0 != "$Root\versions"
        Goto ${Prefix}bad_slot
    ${EndIf}
    Call ${Prefix}NoReparse
    ReadINIStr $0 "$Slot\INSTALL.ini" "Papo" "Owner"
    ${If} $0 != "${PAPO_APP_ID}"
        Goto ${Prefix}bad_slot
    ${EndIf}
    Return
${Prefix}bad_slot:
    MessageBox MB_OK|MB_ICONSTOP "$(Owned)" /SD IDOK
    SetErrorLevel 1
    Quit
FunctionEnd

Function ${Prefix}Closed
    nsExec::Exec /TIMEOUT=10000 '"$Slot\Papo.com" --startup-info'
    Pop $Result
    ${If} $Result == 0
        MessageBox MB_OK|MB_ICONSTOP "$(CloseApp)" /SD IDOK
        SetErrorLevel 1
        Quit
    ${EndIf}
FunctionEnd

Function ${Prefix}Publish
    ClearErrors
    ${If} $Slot == ""
        ReadRegStr $0 HKCU "${ARP}" "PapoOwner"
        ClearErrors
        ${If} $0 == "${PAPO_APP_ID}"
            DeleteRegKey HKCU "${ARP}"
            ${If} ${Errors}
                StrCpy $Failure 1
            ${EndIf}
        ${EndIf}
        ReadRegStr $0 HKCU "${STATE}" "CurrentPath"
        ClearErrors
        ${If} $0 != ""
            DeleteRegValue HKCU "${STATE}" "CurrentPath"
        ${EndIf}
    ${Else}
        Call ${Prefix}AssertSlot
        ReadINIStr $0 "$Slot\INSTALL.ini" "Papo" "Version"
        WriteRegStr HKCU "${STATE}" "Owner" "${PAPO_APP_ID}"
        WriteRegStr HKCU "${STATE}" "CurrentPath" "$Slot"
        WriteRegStr HKCU "${ARP}" "PapoOwner" "${PAPO_APP_ID}"
        WriteRegStr HKCU "${ARP}" "DisplayName" "${PAPO_PROGRAM_NAME}"
        WriteRegStr HKCU "${ARP}" "DisplayVersion" "$0"
        WriteRegStr HKCU "${ARP}" "Publisher" "Papo Contributors"
        WriteRegStr HKCU "${ARP}" "InstallLocation" "$Slot"
        WriteRegStr HKCU "${ARP}" "DisplayIcon" "$Slot\Papo.exe,0"
        WriteRegStr HKCU "${ARP}" "UninstallString" '$\"$Slot\Uninstall.exe$\"'
        WriteRegStr HKCU "${ARP}" "QuietUninstallString" '$\"$Slot\Uninstall.exe$\" /S'
        WriteRegDWORD HKCU "${ARP}" "NoModify" 1
        WriteRegDWORD HKCU "${ARP}" "NoRepair" 1
    ${EndIf}
    ${If} ${Errors}
        StrCpy $Failure 1
    ${EndIf}
FunctionEnd

Function ${Prefix}Recover
    ReadRegStr $Pending HKCU "${STATE}" "PendingPath"
    ${If} $Pending == ""
        Return
    ${EndIf}
    StrCpy $Slot $Pending
    Call ${Prefix}AssertSlot
    Call ${Prefix}Closed
    ReadRegStr $Previous HKCU "${STATE}" "PreviousPath"
    ReadRegStr $PreviousDesktop HKCU "${STATE}" "PreviousDesktop"
    ReadRegStr $0 HKCU "Software\Classes\${PAPO_PROTOCOL}" "InstallPath"
    ${If} $0 != ""
    ${AndIf} $0 != $Pending
    ${AndIf} $0 != $PreviousDesktop
        Goto ${Prefix}recover_failed
    ${EndIf}
    ${If} $PreviousDesktop != ""
        nsExec::Exec /TIMEOUT=30000 '"$PreviousDesktop\Papo.com" --register-desktop'
        Pop $Result
    ${ElseIf} $0 == $Pending
        nsExec::Exec /TIMEOUT=30000 '"$Pending\Papo.com" --unregister-desktop'
        Pop $Result
    ${Else}
        StrCpy $Result 0
    ${EndIf}
    ${If} $Result != 0
        Goto ${Prefix}recover_failed
    ${EndIf}
    StrCpy $Slot $Previous
    StrCpy $Failure 0
    Call ${Prefix}Publish
    ${If} $Failure != 0
        Goto ${Prefix}recover_failed
    ${EndIf}
    DeleteRegValue HKCU "${STATE}" "PendingPath"
    IfErrors ${Prefix}recover_failed
    DeleteRegValue HKCU "${STATE}" "PreviousPath"
    DeleteRegValue HKCU "${STATE}" "PreviousDesktop"
    Return
${Prefix}recover_failed:
    MessageBox MB_OK|MB_ICONSTOP "$(Failed)" /SD IDOK
    SetErrorLevel 1
    Quit
FunctionEnd
!macroend
!insertmacro Common ""
!insertmacro Common "un."

Function .onInit
    Call Initialize
    Call Recover
    ${GetParameters} $Params
    ${GetOptions} "$Params" "/RECOVERONLY" $0
    ${IfNot} ${Errors}
        SetErrorLevel 0
        Quit
    ${EndIf}
    ReadRegStr $Previous HKCU "${STATE}" "CurrentPath"
    ${If} $Previous != ""
        StrCpy $Slot $Previous
        Call AssertSlot
        ReadINIStr $0 "$Slot\INSTALL.ini" "Papo" "Version"
        ${VersionCompare} "${PAPO_VERSION}" "$0" $1
        ${If} $1 == 2
            MessageBox MB_OK|MB_ICONSTOP "$(Downgrade)" /SD IDOK
            SetErrorLevel 1
            Quit
        ${EndIf}
    ${EndIf}
FunctionEnd

Function Prune
    FindFirst $Cursor $Found "$Root\versions\*"
    ${DoWhile} $Found != ""
        ${If} $Found != "."
        ${AndIf} $Found != ".."
        ${AndIf} "$Root\versions\$Found" != $INSTDIR
        ${AndIf} "$Root\versions\$Found" != $Previous
            System::Call 'kernel32::GetFileAttributesW(w "$Root\versions\$Found") i.r0'
            IntOp $0 $0 & 0x400
            ${If} $0 == 0
                ReadINIStr $0 "$Root\versions\$Found\INSTALL.ini" "Papo" "Owner"
                ${If} $0 == "${PAPO_APP_ID}"
                    IfFileExists "$Root\versions\$Found\Uninstall.exe" 0 prune_next
                    ExecWait '"$Root\versions\$Found\Uninstall.exe" /S /RETIRED _?=$Root\versions\$Found' $Result
                    ${If} $Result == 0
                        Delete "$Root\versions\$Found\Uninstall.exe"
                        RMDir "$Root\versions\$Found"
                    ${EndIf}
                ${EndIf}
            ${EndIf}
        ${EndIf}
prune_next:
        FindNext $Cursor $Found
    ${Loop}
    FindClose $Cursor
FunctionEnd

Section "Papo" SEC_MAIN
    ClearErrors
    CreateDirectory "$Root\versions"
    IfErrors install_failed
    GetTempFileName $INSTDIR "$Root\versions"
    IfErrors install_failed
    Delete "$INSTDIR"
    CreateDirectory "$INSTDIR"
    WriteINIStr "$INSTDIR\INSTALL.ini" "Papo" "Owner" "${PAPO_APP_ID}"
    WriteINIStr "$INSTDIR\INSTALL.ini" "Papo" "Version" "${PAPO_VERSION}"
    WriteINIStr "$INSTDIR\INSTALL.ini" "Papo" "ArchiveSHA256" "${PAPO_ARCHIVE_SHA256}"
    StrCpy $Slot $INSTDIR
    Call AssertSlot
    !insertmacro InstallPayload
    ClearErrors
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    IfErrors install_failed
    Call Closed
    nsExec::Exec /TIMEOUT=180000 '"$INSTDIR\Papo.com" --check-packaged-runtime'
    Pop $Result
    ${If} $Result != 0
        Goto install_failed
    ${EndIf}
    ReadRegStr $PreviousDesktop HKCU "Software\Classes\${PAPO_PROTOCOL}" "InstallPath"
    ${If} $PreviousDesktop != ""
        ReadRegStr $0 HKCU "Software\Classes\${PAPO_PROTOCOL}" "PapoOwner"
        ${If} $0 != "${PAPO_APP_ID}"
            Goto install_failed
        ${EndIf}
    ${EndIf}
    ClearErrors
    WriteRegStr HKCU "${STATE}" "Owner" "${PAPO_APP_ID}"
    WriteRegStr HKCU "${STATE}" "PreviousPath" "$Previous"
    WriteRegStr HKCU "${STATE}" "PreviousDesktop" "$PreviousDesktop"
    WriteRegStr HKCU "${STATE}" "PendingPath" "$INSTDIR"
    IfErrors install_failed
    nsExec::Exec /TIMEOUT=30000 '"$INSTDIR\Papo.com" --register-desktop'
    Pop $Result
    ${If} $Result != 0
        Goto rollback
    ${EndIf}
    nsExec::Exec /TIMEOUT=30000 '"$INSTDIR\Papo.com" --desktop-info'
    Pop $Result
    ${If} $Result != 0
        Goto rollback
    ${EndIf}
    StrCpy $Failure 0
    Call Publish
    ${If} $Failure != 0
        Goto rollback
    ${EndIf}
    DeleteRegValue HKCU "${STATE}" "PendingPath"
    IfErrors rollback
    DeleteRegValue HKCU "${STATE}" "PreviousPath"
    DeleteRegValue HKCU "${STATE}" "PreviousDesktop"
    ; EN-US: Keep one previous slot; remove older versions only through their exact inventories.
    ; PT-BR: Preserva uma versão anterior; remove versões mais antigas só pelos inventários exatos.
    Call Prune
    SetErrorLevel 0
    Goto installed
rollback:
    Call Recover
install_failed:
    ; EN-US: A failed or interrupted slot is retained for safe retry/inspection, never recursively removed.
    ; PT-BR: Uma versão incompleta é preservada para inspeção/repetição segura, sem remoção recursiva.
    MessageBox MB_OK|MB_ICONSTOP "$(Failed)" /SD IDOK
    SetErrorLevel 1
    Abort
installed:
SectionEnd

Function un.onInit
    Call un.Initialize
    Call un.Recover
    StrCpy $Slot $INSTDIR
    Call un.AssertSlot
    ReadINIStr $0 "$INSTDIR\INSTALL.ini" "Papo" "ArchiveSHA256"
    ${If} $0 != "${PAPO_ARCHIVE_SHA256}"
        MessageBox MB_OK|MB_ICONSTOP "$(Owned)" /SD IDOK
        SetErrorLevel 1
        Quit
    ${EndIf}
    Call un.Closed
    ${GetParameters} $Params
    StrCpy $Failure 0
    !insertmacro CheckPayload
    ${If} $Failure != 0
        MessageBox MB_OK|MB_ICONSTOP "$(CloseApp)" /SD IDOK
        SetErrorLevel 1
        Quit
    ${EndIf}
    ${GetOptions} "$Params" "/CHECKONLY" $0
    ${IfNot} ${Errors}
        SetErrorLevel 0
        Quit
    ${EndIf}
FunctionEnd

Section "Uninstall"
    ReadRegStr $CurrentOwner HKCU "${STATE}" "CurrentPath"
    ${If} $CurrentOwner == $INSTDIR
        ; EN-US: Check every retained version before changing the active identity.
        ; PT-BR: Verifica todas as versões preservadas antes de alterar a identidade ativa.
        FindFirst $Cursor $Found "$Root\versions\*"
        ${DoWhile} $Found != ""
            ${If} $Found != "."
            ${AndIf} $Found != ".."
            ${AndIf} "$Root\versions\$Found" != $INSTDIR
                System::Call 'kernel32::GetFileAttributesW(w "$Root\versions\$Found") i.r0'
                IntOp $0 $0 & 0x400
                ${If} $0 != 0
                    FindClose $Cursor
                    Goto uninstall_failed
                ${EndIf}
                ReadINIStr $0 "$Root\versions\$Found\INSTALL.ini" "Papo" "Owner"
                ${If} $0 == "${PAPO_APP_ID}"
                    IfFileExists "$Root\versions\$Found\Uninstall.exe" 0 next_check
                    ExecWait '"$Root\versions\$Found\Uninstall.exe" /S /RETIRED /CHECKONLY _?=$Root\versions\$Found' $Result
                    ${If} $Result != 0
                        FindClose $Cursor
                        Goto uninstall_failed
                    ${EndIf}
                ${EndIf}
            ${EndIf}
next_check:
            FindNext $Cursor $Found
        ${Loop}
        FindClose $Cursor
        FindFirst $Cursor $Found "$Root\versions\*"
        ${DoWhile} $Found != ""
            ${If} $Found != "."
            ${AndIf} $Found != ".."
            ${AndIf} "$Root\versions\$Found" != $INSTDIR
                ReadINIStr $0 "$Root\versions\$Found\INSTALL.ini" "Papo" "Owner"
                ${If} $0 == "${PAPO_APP_ID}"
                    IfFileExists "$Root\versions\$Found\Uninstall.exe" 0 next_remove
                    ExecWait '"$Root\versions\$Found\Uninstall.exe" /S /RETIRED _?=$Root\versions\$Found' $Result
                    ${If} $Result != 0
                        FindClose $Cursor
                        Goto uninstall_failed
                    ${EndIf}
                    Delete "$Root\versions\$Found\Uninstall.exe"
                    RMDir "$Root\versions\$Found"
                ${EndIf}
            ${EndIf}
next_remove:
            FindNext $Cursor $Found
        ${Loop}
        FindClose $Cursor
        ; EN-US: An older uninstaller never unregisters a newer or portable owner's identity.
        ; PT-BR: Um desinstalador antigo nunca remove a identidade de outra versão ou de uma cópia portátil.
        ReadRegStr $0 HKCU "Software\Classes\${PAPO_PROTOCOL}" "InstallPath"
        ${If} $0 == $INSTDIR
            nsExec::Exec /TIMEOUT=30000 '"$INSTDIR\Papo.com" --unregister-desktop'
            Pop $Result
            ${If} $Result != 0
                Goto uninstall_failed
            ${EndIf}
        ${EndIf}
    ${EndIf}
    StrCpy $Failure 0
    !insertmacro RemovePayload
    ${If} $Failure != 0
        Goto uninstall_failed
    ${EndIf}
    ${If} $CurrentOwner == $INSTDIR
        ClearErrors
        DeleteRegKey HKCU "${ARP}"
        IfErrors uninstall_failed
        DeleteRegKey HKCU "${STATE}"
        IfErrors uninstall_failed
    ${EndIf}
    Delete "$INSTDIR\INSTALL.ini"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
    RMDir "$Root\versions"
    RMDir "$Root"
    SetErrorLevel 0
    Goto uninstalled
uninstall_failed:
    MessageBox MB_OK|MB_ICONSTOP "$(Failed)" /SD IDOK
    SetErrorLevel 1
    Abort
uninstalled:
SectionEnd
