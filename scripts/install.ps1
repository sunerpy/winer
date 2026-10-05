# Installs winer on Windows: downloads a release's NSIS installer, checks it against the release's
# SHA256SUMS and runs it silently for the current user (no administrator rights needed).
#
#   irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
#
# Pin a version with $env:WINER_VERSION (for example "0.0.1"). The script is ASCII on purpose:
# Windows PowerShell 5.1 decodes a downloaded script without a charset as Latin-1.
$ErrorActionPreference = "Stop"
# Windows PowerShell 5.1 draws a progress bar that slows downloads by an order of magnitude.
$ProgressPreference = "SilentlyContinue"
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = "sunerpy/winer"
$ChecksumFile = "SHA256SUMS"

# x64 Windows, or Windows on ARM, which runs the x64 build.
if ($env:PROCESSOR_ARCHITECTURE -notin @("AMD64", "ARM64")) {
  throw "winer needs 64-bit Windows; this is $env:PROCESSOR_ARCHITECTURE"
}

if ($env:WINER_VERSION) {
  $Version = $env:WINER_VERSION -replace '^v', ''
} else {
  $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" `
    -Headers @{ "User-Agent" = "winer-install" }
  $Version = $Release.tag_name -replace '^v', ''
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "not a version: $Version" }

$Asset = "winer_${Version}_x64-setup.exe"
$BaseUrl = "https://github.com/$Repo/releases/download/v$Version"
$TempDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("winer-" + [Guid]::NewGuid()))

try {
  $Installer = Join-Path $TempDir $Asset
  $Checksums = Join-Path $TempDir $ChecksumFile
  Write-Host "Downloading winer $Version ..."
  Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/$Asset" -OutFile $Installer
  Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/$ChecksumFile" -OutFile $Checksums

  $Line = Get-Content $Checksums | Where-Object { $_ -match ("\s\*?" + [Regex]::Escape($Asset) + "$") } |
    Select-Object -First 1
  if (-not $Line) { throw "$Asset is not listed in $ChecksumFile" }
  $Expected = ($Line -split '\s+')[0].ToLowerInvariant()
  $Actual = (Get-FileHash -Algorithm SHA256 -Path $Installer).Hash.ToLowerInvariant()
  if ($Actual -ne $Expected) { throw "checksum mismatch for $Asset; nothing was installed" }

  Write-Host "Checksum verified. Installing ..."
  $Setup = Start-Process -FilePath $Installer -ArgumentList "/S" -Wait -PassThru
  if ($Setup.ExitCode -ne 0) { throw "the installer exited with code $($Setup.ExitCode)" }
  Write-Host "winer $Version is installed. Start it from the Start menu."
} finally {
  Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
}
