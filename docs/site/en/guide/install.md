# Install

winer needs 64-bit Windows 10 or 11. It uses WebView2, which Windows 10 and 11 normally have; the
installer downloads it when it is missing. winer installs for the current user and needs no
administrator rights to install.

## One command

In PowerShell:

```powershell
irm https://github.com/sunerpy/winer/releases/latest/download/install.ps1 | iex
```

The script downloads the latest installer, checks its SHA-256 against the same release's
`SHA256SUMS` and installs silently only when it matches; otherwise it stops and installs nothing.

To pin a version, set `WINER_VERSION` and use that version's own script:

```powershell
$env:WINER_VERSION = "0.0.1"
irm https://github.com/sunerpy/winer/releases/download/v0.0.1/install.ps1 | iex
```

## Installer

Download `winer_<version>_x64-setup.exe` from
[GitHub Releases](https://github.com/sunerpy/winer/releases) and run it.

The installer has no code-signing certificate, so Windows shows "Windows protected your PC": choose
**More info**, then **Run anyway**. Some antivirus software may flag it, because winer reads another
program's (the client's) local API.

## Verify a download

Every release carries `SHA256SUMS` and a GitHub build attestation for each file. To check a manual
download:

```powershell
Get-FileHash .\winer_0.0.1_x64-setup.exe -Algorithm SHA256
```

Compare the result with the line for that file in `SHA256SUMS`. With the GitHub CLI you can also
confirm that winer's release workflow built the file:

```bash
gh attestation verify winer_0.0.1_x64-setup.exe --repo sunerpy/winer \
  --signer-workflow sunerpy/winer/.github/workflows/release.yml
```

## Update

Check for updates in **Settings › About** and choose **Update now** when one is available: winer
installs it and restarts. Every update is signed, and winer installs only an update whose signature
verifies against the public key built into it.

## Uninstall

Uninstall winer from Windows **Settings › Apps › Installed apps**. Settings live in
`%APPDATA%\app.winer.desktop\settings.json` and logs in `%LOCALAPPDATA%\app.winer.desktop\logs`;
delete them by hand if you no longer need them. Remove the client plugin first from the
**In-client** page, or delete Pengu Loader's `plugins\winer` folder.
