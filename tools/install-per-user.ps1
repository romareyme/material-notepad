# Install Material Notepad for the current user without elevation.
#
# For environments where Inno Setup cannot extract to %TEMP% ("Ошибка 5:
# Отказано в доступе" while creating %TEMP%\is-XXXX.tmp): this script copies the
# files directly, so no temporary directory and no administrator rights are used.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File tools\install-per-user.ps1
#   powershell -ExecutionPolicy Bypass -File tools\install-per-user.ps1 -Uninstall

param(
    [switch]$Uninstall,
    [string]$Source = ''
)

$ErrorActionPreference = 'Stop'

$AppName = 'Material Notepad'
$ExeName = 'material-notepad.exe'
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\$AppName"
$StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MaterialNotepad'
$Types = @('txt', 'log', 'md', 'json', 'ini', 'csv', 'cfg')

if (-not $Source) {
    $Source = Split-Path -Parent $PSScriptRoot
}

function Get-Icon([string]$name) {
    Join-Path $InstallDir "icons\$name.ico"
}

if ($Uninstall) {
    foreach ($ext in $Types) {
        $progId = "MaterialNotepad.$ext"
        Remove-Item -Path "HKCU:\Software\Classes\$progId" -Recurse -Force -ErrorAction SilentlyContinue
        Remove-ItemProperty -Path "HKCU:\Software\Classes\.$ext\OpenWithProgids" -Name $progId -Force -ErrorAction SilentlyContinue
        $current = (Get-ItemProperty "HKCU:\Software\Classes\.$ext" -ErrorAction SilentlyContinue).'(default)'
        if ($current -eq $progId) {
            Remove-ItemProperty -Path "HKCU:\Software\Classes\.$ext" -Name '(default)' -ErrorAction SilentlyContinue
        }
    }
    Remove-Item -Path 'HKCU:\Software\Classes\Applications\material-notepad.exe' -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $UninstallKey -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path (Join-Path $StartMenu "$AppName.lnk") -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Removed $AppName"
    return
}

$exeSource = Join-Path $Source $ExeName
if (-not (Test-Path $exeSource)) { $exeSource = Join-Path $Source "dist\$ExeName" }
if (-not (Test-Path $exeSource)) { $exeSource = Join-Path $Source "target\release\$ExeName" }
if (-not (Test-Path $exeSource)) { throw "Not found: $ExeName (expected next to the script, in dist\ or target\release\)" }

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir 'icons') | Out-Null
Copy-Item $exeSource (Join-Path $InstallDir $ExeName) -Force
foreach ($file in 'README.md', 'LICENSE') {
    $from = Join-Path $Source $file
    if (Test-Path $from) { Copy-Item $from (Join-Path $InstallDir $file) -Force }
}
$iconSource = Join-Path $Source 'assets\icon.ico'
if (Test-Path $iconSource) { Copy-Item $iconSource (Join-Path $InstallDir 'icon.ico') -Force }
$iconsSource = Join-Path $Source 'assets\icons'
if (Test-Path $iconsSource) {
    Copy-Item (Join-Path $iconsSource '*.ico') (Join-Path $InstallDir 'icons') -Force
}

$exe = Join-Path $InstallDir $ExeName

# Start Menu shortcut.
New-Item -ItemType Directory -Force -Path $StartMenu | Out-Null
$shell = New-Object -ComObject WScript.Shell
$lnk = $shell.CreateShortcut((Join-Path $StartMenu "$AppName.lnk"))
$lnk.TargetPath = $exe
$lnk.WorkingDirectory = $InstallDir
$lnk.IconLocation = "$exe,0"
$lnk.Description = $AppName
$lnk.Save()

# Register as an application and give every text type its own ProgID and icon.
New-Item -Path "HKCU:\Software\Classes\Applications\$ExeName" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\Applications\$ExeName" -Name 'FriendlyAppName' -Value $AppName
New-Item -Path "HKCU:\Software\Classes\Applications\$ExeName\DefaultIcon" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\Applications\$ExeName\DefaultIcon" -Name '(default)' -Value "$exe,0"
New-Item -Path "HKCU:\Software\Classes\Applications\$ExeName\shell\open\command" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\Applications\$ExeName\shell\open\command" -Name '(default)' -Value "`"$exe`" `"%1`""

foreach ($ext in $Types) {
    $progId = "MaterialNotepad.$ext"
    $icon = Get-Icon $ext
    if (-not (Test-Path $icon)) { $icon = Join-Path $InstallDir 'icon.ico' }
    New-Item -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId" -Name '(default)' -Value "$AppName - .$ext"
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Name '(default)' -Value $icon
    New-Item -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Name '(default)' -Value "`"$exe`" `"%1`""
    New-Item -Path "HKCU:\Software\Classes\.$ext\OpenWithProgids" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\.$ext\OpenWithProgids" -Name $progId -Value ''
    New-Item -Path "HKCU:\Software\Classes\.$ext\DefaultIcon" -Force | Out-Null
    Set-ItemProperty -Path "HKCU:\Software\Classes\.$ext\DefaultIcon" -Name '(default)' -Value $icon
    Set-ItemProperty -Path "HKCU:\Software\Classes\.$ext" -Name '(default)' -Value $progId
}

# Uninstall entry.
New-Item -Path $UninstallKey -Force | Out-Null
Set-ItemProperty -Path $UninstallKey -Name 'DisplayName' -Value $AppName
Set-ItemProperty -Path $UninstallKey -Name 'DisplayIcon' -Value "$exe,0"
Set-ItemProperty -Path $UninstallKey -Name 'DisplayVersion' -Value '1.0.0'
Set-ItemProperty -Path $UninstallKey -Name 'InstallLocation' -Value $InstallDir
Set-ItemProperty -Path $UninstallKey -Name 'Publisher' -Value $AppName
Set-ItemProperty -Path $UninstallKey -Name 'NoModify' -Value 1 -Type DWord
Set-ItemProperty -Path $UninstallKey -Name 'NoRepair' -Value 1 -Type DWord
Set-ItemProperty -Path $UninstallKey -Name 'UninstallString' -Value "powershell.exe -ExecutionPolicy Bypass -File `"$(Join-Path $InstallDir 'uninstall.ps1')`""

# Ship the uninstaller next to the app.
$self = Join-Path $PSScriptRoot 'install-per-user.ps1'
Copy-Item $self (Join-Path $InstallDir 'uninstall.ps1') -Force

# Refresh the shell icon cache.
& "$env:SystemRoot\System32\ie4uinit.exe" -show 2>$null

Write-Host "Installed $AppName to $InstallDir"
Write-Host "Start Menu shortcut created; text files now use the Material Notepad icons."
Write-Host "Run again with -Uninstall to remove it."
