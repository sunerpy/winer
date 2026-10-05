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

winer checks for updates by itself after it starts and every few hours while it runs. When there is
one, an update button appears at the right of the title bar; open it and choose **Update now**:
winer installs the update and restarts. **Settings › About** checks on demand. Every update is
signed, and winer installs only an update whose signature verifies against the public key built
into it.

## Uninstall

First choose **Turn off the in-client features** on the **In-client** page, which removes the plugin
and the `version.dll` link winer created; then uninstall winer from Windows **Settings › Apps ›
Installed apps**. Settings live in `%APPDATA%\app.winer.desktop\settings.json`, and the logs and the
Pengu Loader winer ships in `%LOCALAPPDATA%\app.winer.desktop`; delete them by hand if you no longer
need them.
