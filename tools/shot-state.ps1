# Capture one UI state per process run, so a missed keystroke cannot silently
# produce a screenshot of the wrong page.
#
#   powershell -ExecutionPolicy Bypass -File tools\shot-state.ps1 -State find
#
# States: editor, find, panel, settings, shortcuts, light, menu

param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('editor', 'find', 'panel', 'settings', 'shortcuts', 'light', 'menu')]
    [string]$State,
    [string]$Out = ''
)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct RS { public int Left, Top, Right, Bottom; }
public struct PS { public int X, Y; }
public class SS {
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RS r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref PS p);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr pid);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool on);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
public static class SFocus {
  public static void Force(IntPtr w) {
    IntPtr fg = SS.GetForegroundWindow();
    uint t = SS.GetWindowThreadProcessId(w, IntPtr.Zero);
    uint f = SS.GetWindowThreadProcessId(fg, IntPtr.Zero);
    uint me = SS.GetCurrentThreadId();
    bool a = f != 0 && SS.AttachThreadInput(me, f, true);
    bool b = t != 0 && t != f && SS.AttachThreadInput(me, t, true);
    SS.BringWindowToTop(w); SS.SetForegroundWindow(w);
    if (b) { SS.AttachThreadInput(me, t, false); }
    if (a) { SS.AttachThreadInput(me, f, false); }
  }
}
"@

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\material-notepad.exe'
$sample = Join-Path $root 'target\sample.rs'
if (-not $Out) { $Out = Join-Path $root "target\shot-$State.png" }

Get-Process material-notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800
Start-Process -FilePath $exe -ArgumentList "`"$sample`"" -RedirectStandardError (Join-Path $root 'target\shot-err.txt') | Out-Null
Start-Sleep -Seconds 4

$p = Get-Process material-notepad | Select-Object -First 1
[SS]::SetWindowPos($p.MainWindowHandle, [IntPtr]::new(-1), 0, 0, 1280, 800, 0x0040) | Out-Null
[SFocus]::Force($p.MainWindowHandle)
Start-Sleep -Milliseconds 900

$keys = @{
    'find'      = '^f'
    'panel'     = '^+f'
    'settings'  = '^,'
    'shortcuts' = '{F1}'
    'menu'      = '{F10}'
    'light'     = '^,'
    'editor'    = ''
}
$sequence = $keys[$State]
if ($sequence) {
    [System.Windows.Forms.SendKeys]::SendWait($sequence)
    Start-Sleep -Milliseconds 1500
}

$h = $p.MainWindowHandle
$c = New-Object RS
[SS]::GetClientRect($h, [ref]$c) | Out-Null
$o = New-Object PS
$o.X = 0; $o.Y = 0
[SS]::ClientToScreen($h, [ref]$o) | Out-Null
$bmp = New-Object System.Drawing.Bitmap $c.Right, $c.Bottom
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($o.X, $o.Y, 0, 0, (New-Object System.Drawing.Size $c.Right, $c.Bottom))
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Host ("saved {0} ({1}x{2})" -f $Out, $c.Right, $c.Bottom)
