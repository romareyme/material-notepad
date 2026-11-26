; Material Notepad installer (Inno Setup 6).
;
; Offers an install path, a Start Menu icon, an "Open in Material Notepad" entry
; in the Explorer context menu and optional per-type file icons. The registry
; entries are written per user, so a per-user install needs no elevation.

#define AppName "Material Notepad"
#define AppVersion "1.1.1"
#define AppPublisher "Material Notepad"
#define AppExeName "material-notepad.exe"
#define AppId "{{FE899CAE-0B69-4120-8FB3-28A656A47897}"

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppCopyright=GPL-3.0-or-later
VersionInfoVersion={#AppVersion}
VersionInfoDescription={#AppName} Setup
VersionInfoProductName={#AppName}
VersionInfoCompany={#AppPublisher}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableDirPage=no
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=MaterialNotepad-{#AppVersion}-setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
AllowNoIcons=yes
UsePreviousAppDir=yes
UsePreviousTasks=yes
CloseApplications=yes
RestartApplications=yes
MinVersion=10.0

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
russian.StartMenuTask=—оздать иконку в меню Ђѕускї
english.StartMenuTask=Create a Start Menu shortcut
russian.FileIconsTask=Ќазначить иконки Material Notepad текстовым файлам (txt, cfg, json и другим)
english.FileIconsTask=Use the Material Notepad document icons for text files (txt, cfg, json and others)
russian.ContextMenuTask=ƒобавить Ђќткрыть в Material Notepadї в контекстное меню ѕроводника
english.ContextMenuTask=Add "Open with Material Notepad" to Explorer context menu
russian.LaunchApp=«апустить Material Notepad
english.LaunchApp=Launch Material Notepad

[Tasks]
Name: "startmenu"; Description: "{cm:StartMenuTask}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce
Name: "fileicons"; Description: "{cm:FileIconsTask}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce
Name: "contextmenu"; Description: "{cm:ContextMenuTask}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce

[Files]
Source: "..\target\release\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icons\*.ico"; DestDir: "{app}\icons"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"; Tasks: startmenu
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"; Tasks: startmenu

[Registry]
; Register the executable as an application so it appears in "Open with".
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExeName}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExeName}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"",0"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExeName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".txt"; ValueData: ""; Flags: uninsdeletekey
; Optional Explorer context menu entry for files and folders (per-user).
Root: HKA; Subkey: "Software\Classes\*\shell\MaterialNotepad"; ValueType: string; ValueName: ""; ValueData: "ќткрыть в Material Notepad"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\*\shell\MaterialNotepad"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExeName}"",0"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\*\shell\MaterialNotepad\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\shell\MaterialNotepad"; ValueType: string; ValueName: ""; ValueData: "ќткрыть папку в Material Notepad"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\shell\MaterialNotepad"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExeName}"",0"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\shell\MaterialNotepad\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""; Tasks: contextmenu; Flags: uninsdeletekey


; Per-type file icons (optional task).
Root: HKA; Subkey: "Software\Classes\.txt\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\icons\txt.ico"; Tasks: fileicons; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.log\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\icons\log.ico"; Tasks: fileicons; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.md\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\icons\md.ico"; Tasks: fileicons; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.json\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\icons\json.ico"; Tasks: fileicons; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.ini\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\icons\ini.ico"; Tasks: fileicons; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.csv\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\icons\csv.ico"; Tasks: fileicons; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.cfg\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\icons\cfg.ico"; Tasks: fileicons; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent
; Windows caches per-extension icons, so make Explorer rebuild them.
Filename: "{sys}\ie4uinit.exe"; Parameters: "-show"; Flags: runhidden nowait; Tasks: fileicons

[Code]
// One ProgID per text type: the ProgID carries both the handler and the badge
// icon. Windows keeps the icon of the ProgID named by
// Explorer\FileExts\<ext>\UserChoice, so a type the user pinned to the bare exe
// (ProgId = Applications\material-notepad.exe) keeps the plain application icon
// until that type is associated with this ProgID once. UserChoice is protected
// by a deny ACE, so no installer can rewrite it.
procedure ApplyTypeIcon(const Extension, Name, Friendly: String);
var
  ProgId, IconPath: String;
begin
  ProgId := 'MaterialNotepad.' + Name;
  IconPath := ExpandConstant('{app}\icons\' + Name + '.ico');

  RegWriteStringValue(HKCU, 'Software\Classes\' + ProgId, '', Friendly);
  RegWriteStringValue(HKCU, 'Software\Classes\' + ProgId + '\DefaultIcon', '', IconPath);
  RegWriteStringValue(HKCU, 'Software\Classes\' + ProgId + '\shell\open\command', '',
    '"' + ExpandConstant('{app}\{#AppExeName}') + '" "%1"');

  // Makes the ProgID selectable in "Open with" as its own entry.
  RegWriteStringValue(HKCU, 'Software\Classes\' + Extension + '\OpenWithProgids', ProgId, '');

  // Applies wherever the type is not pinned to the exe by UserChoice.
  RegWriteStringValue(HKCU, 'Software\Classes\' + Extension, '', ProgId);
end;

procedure RemoveTypeIcon(const Extension, Name: String);
var
  ProgId, Current: String;
begin
  ProgId := 'MaterialNotepad.' + Name;
  RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Classes\' + ProgId);
  RegDeleteValue(HKCU, 'Software\Classes\' + Extension + '\OpenWithProgids', ProgId);
  if RegQueryStringValue(HKCU, 'Software\Classes\' + Extension, '', Current) then
    if CompareText(Current, ProgId) = 0 then
      RegDeleteValue(HKCU, 'Software\Classes\' + Extension, '');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('fileicons') then
  begin
    ApplyTypeIcon('.txt', 'txt', 'Material Notepad Ч текстовый документ');
    ApplyTypeIcon('.cfg', 'cfg', 'Material Notepad Ч файл настроек');
    ApplyTypeIcon('.json', 'json', 'Material Notepad Ч файл JSON');
    ApplyTypeIcon('.md', 'md', 'Material Notepad Ч документ Markdown');
    ApplyTypeIcon('.ini', 'ini', 'Material Notepad Ч файл INI');
    ApplyTypeIcon('.log', 'log', 'Material Notepad Ч журнал');
    ApplyTypeIcon('.csv', 'csv', 'Material Notepad Ч таблица CSV');
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveTypeIcon('.txt', 'txt');
    RemoveTypeIcon('.cfg', 'cfg');
    RemoveTypeIcon('.json', 'json');
    RemoveTypeIcon('.md', 'md');
    RemoveTypeIcon('.ini', 'ini');
    RemoveTypeIcon('.log', 'log');
    RemoveTypeIcon('.csv', 'csv');
  end;
end;
