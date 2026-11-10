; OxyCharacter Windows installer (Inno Setup 6).
;
; Per-user install: no administrator prompt, a Start Menu group, and an
; uninstaller. There is no "portable" marker in this layout, so the editor
; keeps user files in %APPDATA%\OxyCharacter and an uninstall does not delete
; them. The portable zip is built separately and does include the marker.
;
; ISCC /DMyVersion=0.0.2-alpha /DSourceDir=... /DOutDir=... oxycharacter.iss

#ifndef MyVersion
  #define MyVersion "0.0.2-alpha"
#endif
#ifndef SourceDir
  #define SourceDir "staging"
#endif
#ifndef OutDir
  #define OutDir "dist"
#endif

#define MyAppName "OxyCharacter"
#define MyAppVersion "0.0.1"
#define MyAppPublisher "OxyCharacter contributors"
#define MyAppExe "oxycharacter.exe"

[Setup]
AppId={{B3AF2E3F-0BC7-438B-994C-8F84C7FAE68E}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={localappdata}\OxyCharacter
DefaultGroupName=OxyCharacter
DisableProgramGroupPage=yes
DisableDirPage=no
OutputDir={#OutDir}
OutputBaseFilename=OxyCharacter-v{#MyVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExe}
CloseApplications=no
UsedUserAreasWarning=no

[Files]
Source: "{#SourceDir}\oxycharacter.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\oxy.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\makehuman.oxyb"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\INSTALL.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\BUNDLE-REVISION.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\MakeHuman-LICENSE.ASSETS.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#SourceDir}\python\*"; DestDir: "{app}\python"; Flags: ignoreversion recursesubdirs
Source: "{#SourceDir}\plugins\*"; DestDir: "{app}\plugins"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\OxyCharacter"; Filename: "{app}\{#MyAppExe}"; Comment: "Parametric 3D character creator"
Name: "{group}\Uninstall OxyCharacter"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Launch OxyCharacter"; Flags: nowait postinstall skipifsilent
