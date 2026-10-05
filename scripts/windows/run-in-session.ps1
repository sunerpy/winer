# Runs a command in the logged-on user's interactive session (session 1) and waits for it, so GUI
# work (WebView2, window capture) happens where windows exist. Output lands in -Log.
#   pwsh -File run-in-session.ps1 -Command 'pwsh -File C:\winer-dev\capture-window.ps1 ...' -Log C:\winer-dev\out.log
param(
  [Parameter(Mandatory)] [string] $Command,
  [Parameter(Mandatory)] [string] $Log,
  [int] $TimeoutSeconds = 120,
  [switch] $NoWait
)
$ErrorActionPreference = 'Stop'
$name = "winer-" + [Guid]::NewGuid().ToString('N').Substring(0, 8)
$done = "$Log.done"
# cmd's redirection fails before the command runs when the folder is missing.
New-Item -ItemType Directory -Force -Path (Split-Path $Log) | Out-Null
Remove-Item -LiteralPath $Log, $done -ErrorAction SilentlyContinue
$wrapper = Join-Path $env:TEMP "$name.cmd"
Set-Content -LiteralPath $wrapper -Encoding ascii -Value "@echo off`r`n$Command > `"$Log`" 2>&1`r`necho %ERRORLEVEL% > `"$done`""
$user = (Get-CimInstance Win32_ComputerSystem).UserName
schtasks /Create /TN $name /TR "`"$wrapper`"" /SC ONCE /ST 00:00 /RU $user /IT /RL HIGHEST /F | Out-Null
schtasks /Run /TN $name | Out-Null
try {
  if ($NoWait) {
    # Deleting the task before it starts would cancel the run; cmd creates the log as it starts.
    $deadline = (Get-Date).AddSeconds(30)
    while (-not (Test-Path -LiteralPath $Log)) {
      if ((Get-Date) -gt $deadline) { throw "the task did not start within 30 s" }
      Start-Sleep -Milliseconds 200
    }
    return
  }
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  while (-not (Test-Path -LiteralPath $done)) {
    if ((Get-Date) -gt $deadline) { throw "timed out after $TimeoutSeconds s; see $Log" }
    Start-Sleep -Milliseconds 500
  }
  Get-Content -LiteralPath $Log -Raw
  "exit=" + (Get-Content -LiteralPath $done -Raw).Trim()
} finally {
  schtasks /Delete /TN $name /F | Out-Null
}
