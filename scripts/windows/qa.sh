#!/usr/bin/env bash
# Real-machine QA on the Windows host (see AGENTS.md): build, deploy, run in the console session
# with WebView2's DevTools port open, and drive the window over CDP without touching input.
#
#   scripts/windows/qa.sh build [--plain]    cross-build winer.exe (frontend and plugin embedded); `qa` feature unless --plain
#   scripts/windows/qa.sh deploy [--plain]   copy it over and restart it in session 1 (DevTools port open unless --plain)
#   scripts/windows/qa.sh log [lines]        the end of today's log
#   scripts/windows/qa.sh eval '<js>'        evaluate in the window, print the JSON result
#   scripts/windows/qa.sh shot <name> ['<js>']  run the js (if any), then save target/qa/<name>.png
#   scripts/windows/qa.sh lcu <METHOD> <path> ['<json>']  call the client's API there (the token stays there)
#   scripts/windows/qa.sh ps                 run the PowerShell on stdin there
set -euo pipefail
cd "$(dirname "$0")/../.."

SSH=(ssh -F "$HOME/.ssh/windows-local.conf" -o BatchMode=yes -o ConnectTimeout=15)
SCP=(scp -q -F "$HOME/.ssh/windows-local.conf")
HOST=windows-local
DIR='C:\winer-dev'
CDP_PORT=9333

remote_ps() {
  local encoded
  # The single-quoted prelude is PowerShell, whose $ must reach it unexpanded.
  # shellcheck disable=SC2016
  encoded=$({ printf '%s\n' '$ErrorActionPreference = "Stop"; $ProgressPreference = "SilentlyContinue"; [Console]::OutputEncoding = [Text.Encoding]::UTF8'; cat; } | iconv -f UTF-8 -t UTF-16LE | base64 -w0)
  "${SSH[@]}" "$HOST" "pwsh -NoProfile -NonInteractive -EncodedCommand $encoded"
}

case "${1:-}" in
  build)
    features=custom-protocol,qa
    [[ "${2:-}" == --plain ]] && features=custom-protocol
    pnpm --filter @winer/plugin build
    pnpm --filter @winer/desktop build
    cargo xwin build --release --target x86_64-pc-windows-msvc -p winer --features "$features"
    ;;
  deploy)
    remote_ps <<<"New-Item -ItemType Directory -Force -Path '$DIR' | Out-Null"
    "${SCP[@]}" target/x86_64-pc-windows-msvc/release/winer.exe "$HOST:C:/winer-dev/winer.new.exe"
    "${SCP[@]}" scripts/windows/run-in-session.ps1 scripts/windows/capture-window.ps1 scripts/windows/cdp.mjs "$HOST:C:/winer-dev/"
    launch='set WINER_DEVTOOLS_PORT='$CDP_PORT'&& "'$DIR'\winer.exe"'
    wait="try { Invoke-RestMethod -Uri 'http://127.0.0.1:$CDP_PORT/json' -TimeoutSec 2 | Out-Null; 'winer is up, DevTools on $CDP_PORT'; break } catch { Start-Sleep -Milliseconds 500 }"
    if [[ "${2:-}" == --plain ]]; then
      launch='"'$DIR'\winer.exe"'
      wait="if (Get-Process -Name winer -ErrorAction SilentlyContinue) { 'winer is up, no DevTools port'; break } else { Start-Sleep -Milliseconds 500 }"
    fi
    remote_ps <<EOF
Get-Process -Name winer -ErrorAction SilentlyContinue | ForEach-Object { \$_.Kill(); \$_.WaitForExit(5000) | Out-Null }
Move-Item -Force '$DIR\winer.new.exe' '$DIR\winer.exe'
& pwsh -NoProfile -File '$DIR\run-in-session.ps1' -NoWait -Log '$DIR\run.log' -Command '$launch' | Out-Null
\$deadline = (Get-Date).AddSeconds(30)
do {
  $wait
} while ((Get-Date) -lt \$deadline)
if ((Get-Date) -ge \$deadline) { throw 'winer did not come up within 30 s' }
EOF
    ;;
  log)
    remote_ps <<EOF
\$dir = Join-Path \$env:LOCALAPPDATA 'app.winer.desktop\logs'
\$file = Get-ChildItem -LiteralPath \$dir -Filter 'winer*.log' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Get-Content -LiteralPath \$file.FullName -Tail ${2:-40}
EOF
    ;;
  eval)
    script=${2:?javascript expected}
    remote_ps <<EOF
& node '$DIR\cdp.mjs' eval @'
$script
'@
EOF
    ;;
  shot)
    name=${2:?a name expected}
    script=${3:-}
    mkdir -p target/qa
    remote_ps <<EOF
New-Item -ItemType Directory -Force -Path '$DIR\shots' | Out-Null
& node '$DIR\cdp.mjs' shot '$DIR\shots\\$name.png' @'
$script
'@
EOF
    "${SCP[@]}" "$HOST:C:/winer-dev/shots/$name.png" "target/qa/$name.png"
    echo "target/qa/$name.png"
    ;;
  lcu)
    method=${2:?a method expected}
    path=${3:?a path expected}
    body=${4:-}
    remote_ps <<EOF
\$cl = (Get-CimInstance Win32_Process -Filter "Name='LeagueClientUx.exe'").CommandLine
if (-not \$cl) { throw 'LeagueClientUx is not running' }
\$port = [regex]::Match(\$cl, '--app-port=(\d+)').Groups[1].Value
\$token = [regex]::Match(\$cl, '--remoting-auth-token=([^\s"]+)').Groups[1].Value
\$auth = 'Basic ' + [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("riot:\$token"))
\$body = @'
$body
'@
\$request = @{ Method = '$method'; Uri = "https://127.0.0.1:\$port$path"; Headers = @{ Authorization = \$auth }; ContentType = 'application/json' }
if (\$body.Trim()) { \$request.Body = [Text.Encoding]::UTF8.GetBytes(\$body) }
\$response = Invoke-WebRequest @request -SkipCertificateCheck -SkipHttpErrorCheck
"HTTP " + [int]\$response.StatusCode
\$response.Content
EOF
    ;;
  ps)
    remote_ps
    ;;
  *)
    sed -n '2,12p' "$0"
    exit 64
    ;;
esac
