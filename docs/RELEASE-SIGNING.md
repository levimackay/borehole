# Release signing

`.github/workflows/release.yml` builds unsigned artifacts today. No signing
certificates exist yet. The workflow already reads the secrets below and
will start signing automatically, with no workflow changes, the day each
secret is added under **Settings → Secrets and variables → Actions**.

## Updater signing (Tauri)

Signs the update manifest/artifacts so the in-app updater can verify a
release came from this project, not an attacker-controlled server.

| Secret | What it is |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Private key generated via `npm run tauri signer generate`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password on that key, if one was set. |

Without these, `tauri build` produces unsigned updater artifacts (fine for
manual downloads; required before shipping an auto-updater to users).

## macOS code signing + notarization

Without an Apple Developer certificate, macOS builds are ad-hoc signed
(`signingIdentity: "-"` — see `src-tauri/tauri.conf.json`), which avoids the
"app is damaged" Gatekeeper error on Apple Silicon but does not remove the
"unidentified developer" warning, and does not notarize the app.

Requires a paid Apple Developer account ($99/year).

| Secret | What it is |
|---|---|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` export of a "Developer ID Application" certificate. |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting that `.p12`. |
| `APPLE_SIGNING_IDENTITY` | The certificate's keychain identity string (e.g. `Developer ID Application: Name (TEAMID)`). |
| `APPLE_ID` | Apple ID email used for notarization. |
| `APPLE_PASSWORD` | An [app-specific password](https://support.apple.com/en-ca/HT204397) for that Apple ID — never the account password. |
| `APPLE_TEAM_ID` | Apple Developer Team ID. |
| `KEYCHAIN_PASSWORD` | Arbitrary password the workflow uses for the temporary CI keychain it creates — any strong random string, not tied to an Apple account. |

See <https://v2.tauri.app/distribute/sign/macos/> for how to create and
export the certificate.

## Windows code signing

Not currently wired into the workflow — no certificate exists and Windows
builds ship unsigned (triggers a SmartScreen warning on first run). Add this
as a follow-up once a certificate (EV or standard code-signing cert) is
available; see <https://v2.tauri.app/distribute/sign/windows/>.

## Access gaps

All of the above require an Apple Developer account and its credentials,
which this workflow setup does not have and cannot fabricate. Adding them is
a manual step for whoever holds that account.
