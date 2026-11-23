"""Generate installer/MaterialNotepad.iss.

ISCC 6.7 decodes script files with the system ANSI code page and ignores a BOM
(verified: the same script in UTF-8 with and without a BOM renders Cyrillic as
mojibake, while CP1251 renders it correctly). The script is therefore written as
CP1251, which also means every character must be representable in that code page.

Do not rewrite this file with PowerShell `Get-Content`/`Set-Content`: PowerShell
5.1 reads UTF-8 as ANSI and would double-encode the Cyrillic literals.
"""

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
TARGET = ROOT / "installer" / "MaterialNotepad.iss"
CODE_PAGE = "cp1251"

# Extensions that get a context-menu entry and their own icon (assets/icons/<name>.ico).
EXTENSIONS = [".txt", ".log", ".md", ".json", ".ini", ".csv", ".cfg"]

HEADER = r'''; Material Notepad installer (Inno Setup 6).
;
; Offers an install path, a Start Menu icon, an "Open in Material Notepad" entry
; in the Explorer context menu and optional per-type file icons. The registry
; entries are written per user, so a per-user install needs no elevation.

#define AppName "Material Notepad"
#define AppVersion "1.0.0"
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
UsePreviousTasks=no
CloseApplications=yes
RestartApplications=no
MinVersion=10.0

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
russian.StartMenuTask=Создать иконку в меню «Пуск»
english.StartMenuTask=Create a Start Menu shortcut
russian.FileIconsTask=Назначить иконки Material Notepad текстовым файлам (txt, cfg, json и другим)
english.FileIconsTask=Use the Material Notepad document icons for text files (txt, cfg, json and others)
russian.LaunchApp=Запустить Material Notepad
english.LaunchApp=Launch Material Notepad

[Tasks]
Name: "startmenu"; Description: "{cm:StartMenuTask}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce
Name: "fileicons"; Description: "{cm:FileIconsTask}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce

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

'''


# Optional: make Explorer show the document icon with the extension badge.
# A ProgID per type owns both the handler and the icon, so Windows draws the
# badge even when the user once picked the exe in "Open with" (UserChoice).
FILE_ICON = (
    r'Root: HKA; Subkey: "Software\Classes\@EXT@\DefaultIcon"; '
    r'ValueType: string; ValueName: ""; ValueData: "{app}\icons\@NAME@.ico"; '
    r'Tasks: fileicons; Flags: uninsdeletevalue'
    "\n"
)


CODE = r'''
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
    ApplyTypeIcon('.txt', 'txt', 'Material Notepad — текстовый документ');
    ApplyTypeIcon('.cfg', 'cfg', 'Material Notepad — файл настроек');
    ApplyTypeIcon('.json', 'json', 'Material Notepad — файл JSON');
    ApplyTypeIcon('.md', 'md', 'Material Notepad — документ Markdown');
    ApplyTypeIcon('.ini', 'ini', 'Material Notepad — файл INI');
    ApplyTypeIcon('.log', 'log', 'Material Notepad — журнал');
    ApplyTypeIcon('.csv', 'csv', 'Material Notepad — таблица CSV');
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
'''

FOOTER = r'''
[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent
; Windows caches per-extension icons, so make Explorer rebuild them.
Filename: "{sys}\ie4uinit.exe"; Parameters: "-show"; Flags: runhidden nowait; Tasks: fileicons
''' + CODE


def build() -> str:
    parts = [HEADER]
    parts.append("\n; Per-type file icons (optional task).\n")
    for extension in EXTENSIONS:
        name = extension.lstrip(".")
        parts.append(FILE_ICON.replace("@EXT@", extension).replace("@NAME@", name))
    parts.append(FOOTER)
    return "".join(parts)


def main() -> int:
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    text = build()
    TARGET.write_bytes(text.encode(CODE_PAGE))

    check = TARGET.read_bytes().decode(CODE_PAGE)
    assert check == text, "round trip through the code page changed the text"
    assert "@EXT@" not in check and "@NAME@" not in check, "unsubstituted placeholder"
    assert "\ufffd" not in check
    missing = [
        name
        for name in EXTENSIONS
        if not (ROOT / "assets" / "icons" / f"{name.lstrip('.')}.ico").exists()
    ]
    assert not missing, f"missing icons: {missing}"

    cyrillic = [ch for ch in check if 0x0400 <= ord(ch) <= 0x04FF]
    assert len(cyrillic) > 80, f"too little Cyrillic: {len(cyrillic)}"

    # Every extension must point at its own icon, never at a shared one.
    for extension in EXTENSIONS:
        name = extension.lstrip(".")
        expected = f'Software\\Classes\\{extension}\\DefaultIcon'
        line = next((l for l in check.splitlines() if expected in l), None)
        assert line is not None, f"no DefaultIcon entry for {extension}"
        assert f"icons\\{name}.ico" in line, f"{extension} points at the wrong icon: {line}"

    print(f"wrote {TARGET} ({TARGET.stat().st_size} bytes, {CODE_PAGE})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
