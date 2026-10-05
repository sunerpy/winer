# Updater key management

The minisign key that signs winer's automatic updates. This is not Authenticode code signing, which
winer does not have (`docs/accepted-tradeoffs.md`, T-002): an update can be updater-signed while the
installer stays Authenticode-unsigned.

## The key

| Item         | Where it lives                                                                         |
| ------------ | -------------------------------------------------------------------------------------- |
| public key   | `plugins.updater.pubkey` in `app/src-tauri/tauri.conf.json`; key id `F9ABD555ED709F50` |
| private key  | repository secret `TAURI_SIGNING_PRIVATE_KEY`, and the owner's copy                    |
| its password | repository secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and the owner's copy           |

It was made with the Tauri CLI, so its format is the one the bundler expects:

```bash
pnpm tauri signer generate -w ~/.tauri/winer-updater.key
```

`plugins.updater.pubkey` is the `.pub` file's content as written: base64 of the whole public-key
file. Decoding it once more and pasting the inner text produces a config that looks right and a
key of the wrong length.

Never put the private key or its password in the repository, a log, an artifact or a shell
history. `tauri bundle` is the only release step that sees them (`.github/workflows/release.yml`).

## How a release uses it

- `tauri bundle` signs the NSIS installer (`winer_<version>_x64-setup.exe.sig`), because
  `bundle.createUpdaterArtifacts` is on.
- The release's `updater` job decodes the public key and every `.sig` exactly as the updater plugin
  does and verifies them with `minisign` (`.github/scripts/tauri-release.py verify-signatures`). A
  signature made with any other key stops the release there, not on users' machines: a mismatched
  key produces well-formed signatures that every client rejects.
- `latest.json` is written once, after the bundle leg, with the `windows-x86_64` (and
  `windows-x86_64-nsis`) entries pointing at this tag's installer, never at
  `releases/latest/download`. The app asks `releases/latest/download/latest.json`, which follows the
  release marked latest.
- If a signature is missing the release fails as a whole. The updater validates the manifest before
  it compares versions, so a manifest without the running platform's entry would break updates
  for everyone.

## Losing the key

Every installed copy trusts this public key and no other, and verification cannot be turned off.
Without the private key (or its password) no future update can be signed for them, so they stay
on their version for good. Recovery is a new keypair and a manual reinstall:

1. Generate a new keypair, put its public half in `tauri.conf.json` and its private half in the
   secrets.
2. Release normally. Existing copies cannot take this update through the updater.
3. Tell users, in the release notes and the documentation, to download and run the new installer
   once.
4. Record the old key id as retired, so nothing is signed with a recovered copy of it later.

Keep two copies of the private key and its password in two independent places.

## Rotating it on purpose

A client trusts one key at a time, so a rotation needs one hop signed by the old key: release N is
signed with the old key and embeds the new public key; from release N+1 on, sign with the new key.
A copy that updated to N follows; a copy that skipped N needs the manual reinstall.
