; Inno Setup script for Linux Launcher.
; Build: ISCC.exe /DMyAppVersion=2.0.0 assets\Setup.iss  (CI passes the Cargo.toml version)

#define MyAppName "Linux Launcher"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif
#define MyAppPublisher "Confused Engineer"
#define MyAppURL "https://github.com/Confused-Engineer/Linux-Launcher"
#define MyAppExeName "LinuxLauncher.exe"

[Setup]
; AppId uniquely identifies this application; never change it.
AppId={{CD1F8BC3-EB2B-4395-973C-4C554006A5DB}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\LinuxLauncher
DisableProgramGroupPage=yes
; Per-user install; no admin rights needed.
PrivilegesRequired=lowest
OutputBaseFilename=LinuxLauncherSetup
SetupIconFile="favicon.ico"
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma
SolidCompression=yes
WizardStyle=modern
SignTool=signtool $f
; Close a running launcher before upgrading.
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
