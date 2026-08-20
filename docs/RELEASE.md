# TimeBox — Release Runbook

How to produce a distributable TimeBox build. Phase 8 of
[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md).

**Current state:** universal builds work unsigned. Signing and notarization are
**blocked on credentials** — see §3. An unsigned build is dev-only: on another
Mac it is quarantined and Gatekeeper refuses it.

---

## Changelog

| Date (WIB)       | Change                                                                     |
| ---------------- | -------------------------------------------------------------------------- |
| 2026-08-20 14:40 | §1: tray icon is now the exported asset `icons/tray.png` loaded by `platform/tray.rs`, no longer drawn in Rust. Export spec recorded. |
| 2026-08-20 14:10 | §1 rewritten: the icon set is now **exported artwork**, not script output. `icons/generate.py` is no longer the source and must not be re-run — it would overwrite the design. |
| 2026-08-20 10:45 | Notarization credentials **created and validated** against Apple's notary service — the `Developer`-role key authenticates, so 8.4 needs nothing further. Only the Developer ID certificate remains. |
| 2026-08-20 03:55 | §3: use a **dedicated** App Store Connect API key for notarization rather than the shared EAS one, and why. |
| 2026-08-20 03:40 | **Correction:** §3 previously said the account was not enrolled, inferred from Xcode showing no Developer ID certificate. It is enrolled — there are shipped App Store apps. iOS distribution simply never needs a Developer ID certificate, so none was ever created. Notarization switched to the App Store Connect **API key** route, which the existing `.p8` already covers. |
| 2026-08-20 03:20 | Smoke test run: signing configuration **verified** (hardened runtime on, universal preserved, Team ID embedded). Only the certificate is missing. |
| 2026-08-20 03:05 | Recorded the Team ID, how to read an identity off a certificate, and the "no identity found" failure mode. |
| 2026-08-20 02:30 | Recorded the measured 0.1.0 baseline in §4.                                  |
| 2026-08-20 02:10 | Initial version — icon regeneration, universal build, signing, notarization. |

---

## 1. Icon

The icon set is **exported artwork**. The master lives in the design tool, not
in this repo; `src-tauri/icons/` holds the exported result.

> `src-tauri/icons/generate.py` generated the previous, code-drawn icon. It is
> **no longer the source and must not be run** — it overwrites every file below
> with the old `◉` mark. Delete it once you are happy with the exported set.

Export the master as **1024 × 1024 PNG, transparent, sRGB**. macOS does not mask
or round app icons, so the artwork must carry the Apple grid itself: the body
fills **824 × 824 centred** with a **~185 px corner radius**, and the remaining
margin stays transparent for the shadow. Check it at 16 px before committing.

Then regenerate the set from that master:

```bash
npm run tauri icon -- <path-to-master>.png
```

It writes into `src-tauri/icons/` directly, including the four files
`tauri.conf.json` references (`32x32.png`, `128x128.png`, `128x128@2x.png`,
`icon.icns`) plus `icon.png`. The Windows/Android/iOS sizes it also emits are
unused here and can be deleted. Keep the filenames as they are and no config
change is needed.

To verify, build a bundle — `npm run tauri:dev` runs unbundled and shows a
generic Dock icon. A plain `npm run tauri:build` is enough; the universal build
in §2 tells you nothing extra about the icon. macOS caches icons, so run
`killall Dock` if the old one persists.

### The tray icon

The menu bar mark is a **separate asset**, `src-tauri/icons/tray.png`, compiled
into the binary by `platform/tray.rs` — `npm run tauri icon` does not touch it.
Export it alongside the app icon whenever the mark changes:

- **36 × 36 px PNG, RGBA, transparent background** (18pt at 2×)
- the shape **pure black**, inset to roughly **20 × 20** centred
- interior cutouts as **real transparency** — never white fill

macOS treats it as a *template* image: the RGB channels are discarded and the
system repaints the alpha silhouette for light and dark menu bars. So the tray
mark cannot reuse the coloured app artwork, and any white pixel becomes an
opaque blob rather than a hole.

## 2. Universal build

```bash
npm run tauri:build:universal
```

Output: `src-tauri/target/universal-apple-darwin/release/bundle/`
(`macos/TimeBox.app` and `dmg/TimeBox_<version>_universal.dmg`).

Requires both Rust targets:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Verify both architectures actually landed in the binary:

```bash
lipo -archs src-tauri/target/universal-apple-darwin/release/bundle/macos/TimeBox.app/Contents/MacOS/TimeBox
# expected: x86_64 arm64
```

## 3. Signing and notarization — one missing certificate

The account **is** enrolled in the Apple Developer Program (there are shipped
App Store apps). The only missing piece is a **Developer ID Application**
certificate.

It was never created because iOS work does not use one: App Store distribution
signs with *Apple Distribution* certificates, while **Developer ID Application
is Mac-only, for distribution outside the Mac App Store**. Different certificate,
same membership. An empty Developer ID list in Xcode is therefore expected here,
not a sign of a missing membership.

### The one thing to create

Xcode → Settings → Accounts → select the Apple ID → **Manage Certificates** →
**+** → **Developer ID Application**. Then confirm:

```bash
security find-identity -v -p codesigning | grep "Developer ID Application"
```

It will be named `Developer ID Application: Gigih Eristiawan (SQ3B3PDL4S)`.

> **Back up the private key.** Apple caps how many Developer ID Application
> certificates an account may hold, and they are not freely regenerated. Export
> it from Keychain Access as a `.p12` and store it somewhere safe. Never revoke
> one that signed something already distributed.

The **Team ID is `SQ3B3PDL4S`**. It is not a secret — it ships inside the
signature of every notarized app — which is why it is recorded here while keys
and passwords are not.

### Reading these values off a certificate

Do not type an identity from memory; read it back:

```bash
security find-identity -v -p codesigning | grep "Developer ID Application"
```

The Team ID is the certificate's **`OU`** field, which is reliable for every
kind of Apple certificate:

```bash
security find-certificate -c "Apple Development: Gigih Eristiawan" -p \
  | openssl x509 -noout -subject
# subject=UID=…, CN=Apple Development: … (BXFWX492C2), OU=SQ3B3PDL4S, …
#                                          ^ per-cert id     ^ Team ID
```

The parenthetical in the `CN` equals the Team ID **only on Developer ID
certificates**. On an *Apple Development* certificate it is a per-certificate
identifier, and using it as the Team ID is the usual mistake.

### Building signed and notarized

Tauri accepts either of two notarization credentials. **Use the API key** — the
same App Store Connect `.p8` already used for EAS builds. It needs no
app-specific password and works unattended in CI.

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Gigih Eristiawan (SQ3B3PDL4S)"

# App Store Connect API key, dedicated to this project — validated 2026-08-20
export APPLE_API_KEY="747LA9WMKN"
export APPLE_API_ISSUER="1fe1e803-8130-4fb2-90b5-10746a1a3b3a"
export APPLE_API_KEY_PATH="$HOME/.secrets/AuthKey_747LA9WMKN_appstoreconnect.p8"

npm run tauri:build:universal
```

The Key ID and Issuer ID above are identifiers, not credentials — they
authenticate nothing without the `.p8`, which stays outside the repository at
the path shown (mode `600`, in a `700` directory). Delete these two lines if you
would rather keep even the identifiers out of version control.

Confirm the credentials still authenticate at any time, without needing an app
to submit:

```bash
xcrun notarytool history --key "$APPLE_API_KEY_PATH" \
  --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER"
```

`No submission history` on a fresh key means success — it authenticated and
found nothing. An authentication or role error would say so explicitly.

The alternative, if the key is ever unavailable:

```bash
export APPLE_ID="<apple-id-email>"
export APPLE_PASSWORD="<app-specific-password>"   # not the account password
export APPLE_TEAM_ID="SQ3B3PDL4S"
```

Tauri signs with the hardened runtime (required for notarization), submits to
Apple, and staples the ticket when a complete set of either group is present.
With neither, it signs and skips notarization with a warning rather than failing.

**Keep the `.p8` out of the repository.** Point `APPLE_API_KEY_PATH` at a file
outside the working tree; a key committed to git must be revoked.

### Use a key dedicated to this project

Do **not** reuse the `EAS CI / eas-credentials` key. Nothing breaks technically
if you do — API keys are stateless, and notarizing does not consume anything EAS
depends on — but the two would then share a fate: revoking a leaked key would
take down the release pipeline of a shipped App Store app at the same moment.
A separate key keeps an incident here from reaching anything that ships.

Create it in App Store Connect → Users and Access → Integrations → **Team Keys**
(not *Individual Keys*, so it keeps the same issuer), named for this project.

- **Role:** `Developer` is the least-privilege option that notarizes. The EAS
  key's `Admin` is far more than this needs. If submission returns an
  authentication error, create a second key with `App Manager` — a key's access
  **cannot be widened after creation**, so the narrow one is discarded, not
  repaired. Keys are cheap; the account allows 50.
- **The `.p8` downloads once.** Lose it and the key is dead. Store it outside
  the working tree with tight permissions:

  ```bash
  mkdir -p ~/.appstoreconnect/private_keys && chmod 700 ~/.appstoreconnect/private_keys
  mv ~/Downloads/AuthKey_XXXXXXXXXX.p8 ~/.appstoreconnect/private_keys/
  chmod 600 ~/.appstoreconnect/private_keys/AuthKey_XXXXXXXXXX.p8
  ```

- `APPLE_API_KEY` is the 10-character **Key ID** from the key list;
  `APPLE_API_ISSUER` is the **Issuer ID** shown above the list, shared by every
  team key.

TimeBox needs **no entitlements file**. It is not sandboxed — Developer ID
distribution does not require the sandbox — and it uses no capability that the
hardened runtime blocks. Adding entitlements speculatively is how notarization
starts failing for no reason.

### `no identity found`

```
Developer ID Application: Gigih Eristiawan (SQ3B3PDL4S): no identity found
failed to bundle project: failed codesign application
```

The identity string is correct in form — that is exactly what the certificate
will be called once it exists — but no such certificate is in the keychain yet.
This is the membership gate above, not a configuration error. Confirm with
`security find-identity -v -p codesigning`: if the list has no
`Developer ID Application` line, there is nothing to sign with.

### Smoke-testing the pipeline without a membership

The signing path can be exercised with an *Apple Development* certificate. This
proves the configuration works; it produces nothing distributable.

```bash
env -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID \
  APPLE_SIGNING_IDENTITY="Apple Development: Gigih Eristiawan (BXFWX492C2)" \
  npm run tauri:build:universal
```

Unsetting the notarization variables matters: with them present Tauri submits to
Apple, which rejects a Development certificate and fails the build.

The result signs and verifies locally, and is still refused by Gatekeeper on any
other Mac. Only a Developer ID build that has been notarized and stapled is
distributable.

**Run 2026-08-20 — passed.** What it established, none of which changes when the
certificate does:

```
Format=app bundle with Mach-O universal (x86_64 arm64)   ← universal survives signing
CodeDirectory … flags=0x10000(runtime)                   ← hardened runtime ON
Authority=Apple Development: Gigih Eristiawan (BXFWX492C2)
TeamIdentifier=SQ3B3PDL4S                                ← Team ID embedded correctly
```

`codesign --verify --deep --strict` → *valid on disk, satisfies its Designated
Requirement*. The `.dmg` is signed too. Tauri skipped notarization with an
explicit warning rather than failing, which is the correct behaviour when the
notarization variables are absent.

`spctl -a -t install` → **rejected**, `origin=Apple Development: …`. Expected:
Gatekeeper accepts only a notarized Developer ID build. That single line is the
whole remaining gap.

The hardened runtime being on is the load-bearing result — it is the usual cause
of a first notarization rejection, and it is already correct.

### Verifying the result

```bash
APP=src-tauri/target/universal-apple-darwin/release/bundle/macos/TimeBox.app
codesign --verify --deep --strict --verbose=2 "$APP"
spctl -a -vvv -t install "$APP"          # expect: accepted, source=Notarized Developer ID
xcrun stapler validate "$APP"            # expect: The validate action worked!
```

The real test is a *different* Mac: copy the DMG across, open it, and confirm no
Gatekeeper warning. A build that passes `spctl` locally can still be quarantined
elsewhere if the ticket was not stapled.

## 4. Performance check

The app must be invisible when it is not working. Launch the built `.app`, leave
it `IDLE`, and measure:

```bash
ps -o rss=,%cpu=,comm= -p "$(pgrep -x TimeBox)"
```

Measured baseline, universal 0.1.0 (`.app` 9.7 MB, DMG 5.3 MB):

| State | RSS | CPU |
|---|---|---|
| `IDLE` | 54.6 MB | 0.00% over 30 s |
| `RUNNING` | 41.9 MB | 3.28% over 25 s |

To measure `IDLE` without disturbing a live timer, launch the binary directly
with `HOME` pointed at a scratch directory — it gets its own empty database:

```bash
HOME=/tmp/timebox-perf .../TimeBox.app/Contents/MacOS/timebox &
```

Targets: **< 80 MB** resident, **~0% CPU** when `IDLE` / `PAUSED` /
`AWAITING_DECISION`. Idle CPU is the meaningful one — the tick thread parks on a
condvar in those states, so anything above noise means the parking regressed.
Measure again while `RUNNING`: one wakeup a second is expected.

## 5. Acceptance pass

Acceptance tests 1–20 in [SPEC.md](SPEC.md) §12, run by hand on the notarized
build. Tests 9, 14, and 15 encode the product thesis — if any of them fail, do
not ship.
