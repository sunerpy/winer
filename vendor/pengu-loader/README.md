# Pengu Loader core

`core.dll` is the in-client half of [Pengu Loader](https://github.com/PenguLoader/PenguLoader)
v1.1.6 (`1.1.6+4d641f52`, tag commit `4d641f52bc5d70aac4c09dfa1fa7a043a9069aff`), taken unchanged
from the release asset `pengu-loader-v1.1.6.zip`. It is MIT licensed (`LICENSE`, from the same
tag) and signed by SignPath Foundation (Authenticode, timestamped).

```text
sha256  d56fcf8f182dd5d392214eb989869c53e6cb7e0bc2a2d111ea0a3b9d3c0a506f  core.dll
```

winer embeds it (`app/src-tauri/src/lib.rs`, `PENGU_CORE`), writes it to its own data folder and
links the client's `version.dll` to it when no loader is linked yet; Pengu Loader's own window
(`Pengu Loader.exe` and its .NET libraries) is not shipped. The `core_dll_is_the_vendored_release`
test in `app/src-tauri/src/plugin_host.rs` pins the hash above.

To move to another release: download its zip from the release page, check the signature
(`Get-AuthenticodeSignature core.dll` on Windows), replace `core.dll` and `LICENSE`, and update the
version, commit and hash here, in the test and in `docs/platform-notes.md`.
