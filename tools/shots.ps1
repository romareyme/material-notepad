# Rebuild the README screenshots from the running application.
# Keyboard only: other windows can sit above the app, so synthetic clicks are not
# reliable. Each scheme is forced through settings.json before launching.

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct RECT { public int Left, Top, Right, Bottom; }
public struct POINT { public int X, Y; }
public class Shots {
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
}
"@

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\material-notepad.exe'
$out = Join-Path $root 'docs\screenshots'
$settings = Join-Path $env:APPDATA 'MaterialNotepad\settings.json'
$utf8 = New-Object System.Text.UTF8Encoding($false)
New-Item -ItemType Directory -Force -Path $out | Out-Null

function Write-Settings([bool]$dark) {
    $dir = Split-Path -Parent $settings
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $json = @{
        dark_mode        = $dark
        seed             = @(103, 80, 164)
        ui_scale         = 1.0
        editor_font_size = 14.0
        tab_width        = 4
        spaces_for_tabs  = $true
        word_wrap        = $false
        line_numbers     = $true
        restore_session  = $true
        recent           = @()
        last_folder      = $null
    } | ConvertTo-Json -Depth 6
    [System.IO.File]::WriteAllText($settings, $json, $utf8)
}

function Stop-App {
    Get-Process material-notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 900
}

function Start-App([bool]$dark) {
    Stop-App
    Write-Settings $dark
    $sample = Join-Path $root 'target\sample.rs'
    $arguments = if (Test-Path $sample) { @("`"$sample`"") } else { @() }
    Start-Process -FilePath $exe -ArgumentList $arguments -RedirectStandardError (Join-Path $root 'target\shots-err.txt') | Out-Null
    Start-Sleep -Seconds 4
    $p = Get-Process material-notepad | Select-Object -First 1
    [Shots]::SetWindowPos($p.MainWindowHandle, [IntPtr]::new(-1), 0, 0, 1280, 800, 0x0040) | Out-Null
    [Shots]::SetCursorPos(1910, 1070) | Out-Null
    Start-Sleep -Milliseconds 1000
}

function Shot([string]$name) {
    $p = Get-Process material-notepad | Select-Object -First 1
    $h = $p.MainWindowHandle
    $client = New-Object RECT
    [Shots]::GetClientRect($h, [ref]$client) | Out-Null
    $origin = New-Object POINT
    $origin.X = 0
    $origin.Y = 0
    [Shots]::ClientToScreen($h, [ref]$origin) | Out-Null
    $w = $client.Right - $client.Left
    $hh = $client.Bottom - $client.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $w, $hh))
    $g.Dispose()
    $bmp.Save((Join-Path $out "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $bg = $bmp.GetPixel(600, 300)
    $bmp.Dispose()
    Write-Host ("saved {0}.png ({1}x{2}) bg=R{3} G{4} B{5}" -f $name, $w, $hh, $bg.R, $bg.G, $bg.B)
}

function Keys([string]$seq, [int]$wait = 700) {
    [System.Windows.Forms.SendKeys]::SendWait($seq)
    Start-Sleep -Milliseconds $wait
}

# Dark scheme: editor, menu, find bar, find in files, preferences, shortcuts.
Start-App $true
Shot 'dark-editor'

Keys '{F10}'
Shot 'menu'
Keys '{ESC}'

Keys '^f'
Keys 'inventory'
Shot 'find'
Keys '{ESC}'

Keys '^+f'
Shot 'panel'
Keys '{ESC}'

Start-App $true
Keys '^,'
Shot 'settings'
Keys '{ESC}'

Keys '{F1}'
Shot 'shortcuts'
Keys '{ESC}'

# Light scheme: one clean editor shot.
Start-App $false
Shot 'light-theme'

# Leave the user's configuration in dark mode.
Stop-App
Write-Settings $true
Write-Host 'done'

