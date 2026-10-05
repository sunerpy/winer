# Saves a PNG of a process's main window without touching focus (PrintWindow, full content).
# Must run in the interactive session (session 1): over SSH a process sees none of its windows.
#   pwsh -File capture-window.ps1 -Process winer -Out C:\winer-dev\shots\winer.png
param(
  [Parameter(Mandatory)] [string] $Process,
  [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class WinerCapture {
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc proc, IntPtr lParam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int max);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
$pids = @(Get-Process -Name $Process -ErrorAction Stop | ForEach-Object Id)
$best = [IntPtr]::Zero; $bestArea = 0
$callback = [WinerCapture+EnumProc]{
  param($hwnd, $lparam)
  $owner = 0; [void][WinerCapture]::GetWindowThreadProcessId($hwnd, [ref]$owner)
  if ($pids -contains [int]$owner -and [WinerCapture]::IsWindowVisible($hwnd)) {
    $rect = New-Object WinerCapture+RECT; [void][WinerCapture]::GetWindowRect($hwnd, [ref]$rect)
    $title = New-Object System.Text.StringBuilder 256; [void][WinerCapture]::GetWindowText($hwnd, $title, 256)
    $area = ($rect.Right - $rect.Left) * ($rect.Bottom - $rect.Top)
    if ($title.Length -gt 0 -and $area -gt $script:bestArea) { $script:best = $hwnd; $script:bestArea = $area }
  }
  return $true
}
[void][WinerCapture]::EnumWindows($callback, [IntPtr]::Zero)
if ($best -eq [IntPtr]::Zero) { throw "no visible titled window for $Process" }
$rect = New-Object WinerCapture+RECT; [void][WinerCapture]::GetWindowRect($best, [ref]$rect)
$width = $rect.Right - $rect.Left; $height = $rect.Bottom - $rect.Top
$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()
# PW_RENDERFULLCONTENT (2): DirectComposition content (WebView2, CEF) instead of a black frame.
$ok = [WinerCapture]::PrintWindow($best, $hdc, 2)
$graphics.ReleaseHdc($hdc); $graphics.Dispose()
New-Item -ItemType Directory -Force -Path (Split-Path $Out) | Out-Null
$bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $bitmap.Dispose()
"captured ok=$ok size=${width}x${height} session=$((Get-Process -Id $PID).SessionId)"
