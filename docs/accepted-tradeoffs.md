# Accepted tradeoffs

Limitations this project has chosen to live with, so nobody reopens them. Each is a capability that
is not available from inside this repository; each says what is lost, why it cannot be fixed here,
what was rejected, and what would let the entry be removed.

## T-001 — Windows 11 Snap Layouts flyout, lost to the self-drawn title bar

**What is lost.** Hovering the maximize button on Windows 11 normally opens the Snap Layouts
flyout. With `decorations: false` in `app/src-tauri/tauri.conf.json` that flyout never appears.

**What is not lost.** Folklore says `decorations: false` strips `WS_CAPTION` / `WS_THICKFRAME` and
breaks snapping, resizing, shadows and rounded corners. It does not on current tao, which keeps
`WS_CAPTION` (and `WS_SIZEBOX` while resizable) and absorbs the non-client area by answering
`WM_NCCALCSIZE` with 0:

| Feature                                  | Under `decorations: false`                                                                    |
| ---------------------------------------- | --------------------------------------------------------------------------------------------- |
| Aero Snap — drag to an edge, `Win`+arrow | kept: `startDragging()` posts `WM_NCLBUTTONDOWN` with `HTCAPTION`, a standard title-bar drag  |
| Drop shadow and rounded corners          | kept by `"shadow": true`; `shadow: false` would lose both together                            |
| Resize borders and hit-testing           | kept: Tauri lays transparent child windows over the webview, which swallows the parent's hits |

**Why it cannot be fixed here.** The flyout opens only in response to `WM_NCHITTEST`, which is not
delivered for clicks inside the webview. The fix is Window Controls Overlay in WebView2, upstream of
Tauri.

**What was rejected.** `tauri-plugin-decorum`, which fakes the flyout by synthesizing `Win+Z`
through `enigo`: the flyout lands in the wrong place while the resize-border child windows exist,
and the plugin has been unmaintained since late 2024.

**Why the bar is self-drawn at all.** The window lives in the tray and its close button hides it
(`app/src-tauri/src/window.rs`), and the bar has to match the window's own themes.

**What would let this entry be removed.** WebView2 shipping Window Controls Overlay and Tauri
exposing it.

## T-002 — The Windows installer is not Authenticode-signed

**What is lost.** No code-signing certificate is available to this project, so the NSIS installer
and `winer.exe` carry no Authenticode signature:

| Surface                     | Without a signature                                                                        |
| --------------------------- | ------------------------------------------------------------------------------------------ |
| SmartScreen (Windows 10/11) | "Windows protected your PC"; running it takes _More info_ → _Run anyway_                   |
| UAC                         | "Unknown publisher"                                                                        |
| Antivirus engines           | false positives are likely: an app that reads another process's local API looks suspicious |
| Browser download            | Edge or Chrome may call the installer "not commonly downloaded"                            |

Reputation is per binary and per signature, so every unsigned release earns it again from zero.

**What is not lost.** The update trust root. Tauri's updater checks a minisign signature over each
update against the public key compiled into the app (`docs/updater-key-management.md`); that does
not need an Authenticode certificate. "Unsigned" here means Authenticode only.

**Why it cannot be fixed here.** A certificate is issued to a legal identity after paid vetting. A
self-signed one is worse than none: it adds an untrusted-root error and earns no reputation.

**What was rejected.** Self-signing, for that reason; a bare `winer.exe` instead of an installer,
which trades one warning for losing the Start menu entry, the uninstaller and the per-user install
location.

**Mitigations, cheapest first.** An open-source certificate programme (SignPath Foundation, Certum
open source), which needs a public repository and a reproducible build; a paid OV certificate,
which still has to earn reputation, or an EV certificate, which starts with it; and a
false-positive report to each engine that flags a release.

**What would let this entry be removed.** A certificate, and the bundle signed inside
`tauri bundle` (`bundle.windows.signCommand`) so the updater's signature covers the signed bytes.

**What the release can honestly claim.** Every release publishes `SHA256SUMS` and a GitHub artifact
attestation for each asset, which prove a download is what the release workflow built. Neither
claims a publisher identity.
