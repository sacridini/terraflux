param(
  [string]$Model = "",
  [string]$Work = "",
  [string]$Out = "shot.png",
  [int]$Wait = 4,
  [string]$Actions = "",
  [int]$W = 1280,
  [int]$Height = 800
)
# Screenshots of the Terraflux application, for the getting-started tutorials and the quickstart (kept in
# app-shots/; tour.py crops and numbers them). WARNING: it takes over the mouse and the keyboard of the desktop
# while it runs; run it only on a computer nobody is using.
# Opens the Terraflux application (its settings and layout in a scratch folder), runs a list of actions
# (click:x,y | dbl:x,y | rclick:x,y | key:{F5} | sleep:ms | shot:name.png), relative to the window's client
# top-left corner, and saves screenshots of the window's client area.
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hh, bool r);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(int f, int x, int y, int d, int e);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  public static IntPtr Find(int pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { int p; GetWindowThreadProcessId(h, out p); RECT r; GetClientRect(h, out r);
      if (p == pid && IsWindowVisible(h) && r.Right > 200) { found = h; return false; } return true; }, IntPtr.Zero);
    return found;
  }
  public struct RECT { public int Left, Top, Right, Bottom; }
  public struct POINT { public int X, Y; }
}
"@
[Win]::SetProcessDPIAware() | Out-Null
$root = Resolve-Path (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "..\..\..")
if ($Work -eq "") { $Work = Join-Path $root "website\.cache\work\app-shots" }
New-Item -ItemType Directory -Force (Join-Path $Work "appdata") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $Work "local") | Out-Null
$here = $Work
# the application's settings, layout and caches in the scratch folder, not the user's
$env:APPDATA = Join-Path $Work "appdata"
$env:LOCALAPPDATA = Join-Path $Work "local"
$exe = Join-Path $root "target\release\terraflux.exe"
$argv = @("app")
if ($Model -ne "") { $argv += $Model }
$p = Start-Process -FilePath $exe -ArgumentList $argv -PassThru -WorkingDirectory $here
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 60; $i++) {
  Start-Sleep -Milliseconds 250
  $p.Refresh()
  $hwnd = [Win]::Find($p.Id)
  if ($hwnd -ne [IntPtr]::Zero) { break }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Output "no window"; Stop-Process -Id $p.Id; exit 1 }
[Win]::MoveWindow($hwnd, 0, 0, $W, $Height, $true) | Out-Null
Start-Sleep -Seconds 2
[Win]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 500

function Origin {
  $pt = New-Object Win+POINT
  [Win]::ClientToScreen($hwnd, [ref]$pt) | Out-Null
  return $pt
}
function Shot([string]$name) {
  $c = New-Object Win+RECT
  [Win]::GetClientRect($hwnd, [ref]$c) | Out-Null
  $o = Origin
  $bmp = New-Object System.Drawing.Bitmap ($c.Right), ($c.Bottom)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($o.X, $o.Y, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $here $name), [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output "saved $name $($c.Right) x $($c.Bottom) at $($o.X),$($o.Y)"
}
function Click([string]$xy, [int]$down, [int]$up, [int]$times) {
  $o = Origin
  $q = $xy.Split(",")
  [Win]::SetCursorPos($o.X + [int]$q[0], $o.Y + [int]$q[1]) | Out-Null
  Start-Sleep -Milliseconds 150
  for ($t = 0; $t -lt $times; $t++) {
    [Win]::mouse_event($down, 0, 0, 0, 0); Start-Sleep -Milliseconds 40
    [Win]::mouse_event($up, 0, 0, 0, 0); Start-Sleep -Milliseconds 60
  }
}
if ($Actions -ne "") {
  foreach ($a in $Actions.Split("|")) {
    $kv = $a.Split(":", 2)
    $k = $kv[0]; $v = $kv[1]
    switch ($k) {
      "sleep" { Start-Sleep -Milliseconds ([int]$v) }
      "click" { Click $v 2 4 1 }
      "dbl" { Click $v 2 4 2 }
      "rclick" { Click $v 8 16 1 }
      "move" { $o = Origin; $q = $v.Split(","); [Win]::SetCursorPos($o.X + [int]$q[0], $o.Y + [int]$q[1]) | Out-Null }
      "key" { [System.Windows.Forms.SendKeys]::SendWait($v) }
      "drag" {
        $o = Origin; $q = $v.Split(",")
        [Win]::SetCursorPos($o.X + [int]$q[0], $o.Y + [int]$q[1]) | Out-Null; Start-Sleep -Milliseconds 200
        [Win]::mouse_event(2, 0, 0, 0, 0); Start-Sleep -Milliseconds 150
        for ($s = 1; $s -le 12; $s++) {
          $x = [int]$q[0] + ([int]$q[2] - [int]$q[0]) * $s / 12; $y = [int]$q[1] + ([int]$q[3] - [int]$q[1]) * $s / 12
          [Win]::SetCursorPos($o.X + $x, $o.Y + $y) | Out-Null; Start-Sleep -Milliseconds 40
        }
        Start-Sleep -Milliseconds 200; [Win]::mouse_event(4, 0, 0, 0, 0)
      }
      "shot" { Shot $v }
    }
    Start-Sleep -Milliseconds 300
  }
}
Start-Sleep -Seconds $Wait
Shot $Out
Stop-Process -Id $p.Id
