; SecondWind host installer.
;
; Bundles: the companion, the PowerShell glue scripts, and the third-party
; host binaries listed in THIRD-PARTY.md (Apollo installer, usbip-win2,
; xpra client). Build with Inno Setup 6 on a machine where `staging\` has
; been populated per THIRD-PARTY.md.

#define AppName "SecondWind"
#define AppVersion "0.5.0"
#define AppPublisher "SecondWind"

[Setup]
AppId={{7A3F2C9B-5E64-4D1B-9C2A-SECONDWIND01}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\SecondWind
DefaultGroupName=SecondWind
OutputBaseFilename=SecondWind-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64compatible

[Files]
; First-party.
Source: "staging\secondwind-companion.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\scripts\windows\*.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion

; Third-party host binaries (see THIRD-PARTY.md for pinning + licenses).
Source: "staging\apollo\*"; DestDir: "{app}\apollo"; Flags: recursesubdirs ignoreversion
Source: "staging\usbip\*"; DestDir: "{app}\usbip"; Flags: recursesubdirs ignoreversion
Source: "staging\xpra\*"; DestDir: "{app}\xpra"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{group}\SecondWind"; Filename: "{app}\secondwind-companion.exe"
Name: "{autostartup}\SecondWind"; Filename: "{app}\secondwind-companion.exe"

[Run]
; Apollo: silent install (its own installer handles the virtual display
; driver). The companion manages all Apollo configuration afterwards.
Filename: "{app}\apollo\apollo-installer.exe"; Parameters: "/S"; \
    StatusMsg: "Installing the screen engine…"; Flags: waituntilterminated
; usbip-win2 driver setup (one-time driver trust; see docs/USB-SETUP.md).
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\usbip\classic_setup.ps1"""; \
    StatusMsg: "Installing the USB driver…"; Flags: waituntilterminated
; Explorer context menu (per-user, no elevation needed at run time).
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\scripts\Register-SecondWindContextMenu.ps1"" -CompanionPath ""{app}\secondwind-companion.exe"""; \
    StatusMsg: "Adding Explorer shortcuts…"; Flags: waituntilterminated runasoriginaluser
; Launch the companion when setup finishes.
Filename: "{app}\secondwind-companion.exe"; Flags: postinstall nowait skipifsilent

[UninstallRun]
Filename: "powershell.exe"; \
    Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\scripts\Register-SecondWindContextMenu.ps1"" -CompanionPath ""{app}\secondwind-companion.exe"" -Unregister"; \
    Flags: waituntilterminated runasoriginaluser; RunOnceId: "SecondWindContextMenu"
