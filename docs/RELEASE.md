# TimeBox — Release Runbook

How to produce a distributable TimeBox build. Phase 8 of
[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md).

**Current state:** **0.1.0 is published.** The signed, notarized, stapled
universal DMG passes every check in `./scripts/verify-release.sh` (app and DMG
both) and is attached to the [v0.1.0
release](https://github.com/gigiheristiawan/timebox/releases/tag/v0.1.0); the
landing page is live at <https://gigiheristiawan.github.io/timebox/>. This
runbook is now the procedure for the *next* release.

---

## Changelog

| Date (WIB)       | Change                                                                     |
| ---------------- | -------------------------------------------------------------------------- |
| 2026-08-21 14:00 | **Correction to §7.3:** the MAS .pkg cannot be installed and launched locally — Gatekeeper rejects the 3rd Party Mac Developer signature and the profile authorises no devices, so `open` fails with a launchd spawn error. The previous "install the .pkg, then check the container" instruction was wrong. `scripts/sandbox-smoketest.sh` added for local sandbox verification, and the installer's bundle-relocation trap recorded. |
| 2026-08-21 09:35 | §7.1: the 0.1.0 `LaunchAgents/TimeBox.plist` is deleted at startup — an upgrade-path bug, since every 0.1.0 user who enabled launch-at-login has one and nothing else would remove it. |
| 2026-08-21 09:10 | §7.1: recorded that `SMAppService` can refuse where the LaunchAgent plist could not, and the reconcile-at-launch + `launchAtLoginActive` handling that makes the refusal visible instead of silent. |
| 2026-08-20 22:30 | **§7 added — Mac App Store**, a second channel beside the DMG. Records the three source changes that made it possible (private API dropped, `SMAppService` login item, `RunEvent::Reopen`), the one-time certificates/profile setup, `scripts/build-mas.sh`, and that the sandboxed build starts with an empty database because its container path differs. |
| 2026-08-20 20:55 | **§3 redacted for the public repo:** the App Store Connect Key ID and Issuer ID are replaced by `<key-id>` / `<issuer-id>` placeholders — the Issuer ID is account-wide, not per-project, so it does not belong in a public repository. Real values live in `~/.secrets/` next to the `.p8`. Note that both appear in commits before this one; history was not rewritten. |
| 2026-08-20 18:10 | §6: `--notes-file docs/release-notes/0.1.0.md` restored to the release command now that the file exists, and the one-file-per-release convention recorded. |
| 2026-08-20 17:30 | **`tauri build` notarizes the `.app` but not the DMG** — observed on the first Developer ID build: app `Notarized Developer ID` and stapled, DMG `Unnotarized Developer ID` with no ticket. §3 now carries the `notarytool submit` + `stapler staple` step for the container, and verification moved to `scripts/verify-release.sh`, which checks stapling on **both** artifacts. |
| 2026-08-20 16:45 | **Unblocked:** the Developer ID Application certificate exists — `security find-identity` lists it. Supersedes every "blocked on credentials" line above and the old §3 title *one missing certificate*; §3 now reads as procedure, not a blocker. |
| 2026-08-20 16:20 | §6 added — publishing: the GitHub release the download button points at, and Pages serving `docs/`. Icon paths in `docs/index.html` vendored under `docs/` so they survive publication. |
| 2026-08-20 14:55 | The whole `src-tauri/icons/` directory is tracked, Windows/Android/iOS output included — §1 previously said to delete it. |
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
`icon.icns`) plus `icon.png`. Keep the filenames as they are and no config
change is needed.

It also emits Windows/Android/iOS sizes. This build does not use them, but the
whole directory is committed anyway: the tool regenerates all of it in one pass,
so tracking everything keeps `git status` clean after a re-run and means the
next platform target needs no second export. Commit the full output.

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

## 3. Signing and notarization

The account **is** enrolled in the Apple Developer Program (there are shipped
App Store apps), and the **Developer ID Application** certificate now exists —
`Developer ID Application: Gigih Eristiawan (SQ3B3PDL4S)`, confirmed in the
login keychain on 2026-08-20. Both halves are in place; skip to *Building signed
and notarized* below. The rest of this section is why it was missing for so
long, and how to recreate it on another machine.

It was never created because iOS work does not use one: App Store distribution
signs with *Apple Distribution* certificates, while **Developer ID Application
is Mac-only, for distribution outside the Mac App Store**. Different certificate,
same membership. An empty Developer ID list in Xcode is therefore expected here,
not a sign of a missing membership.

### Creating it (done — kept for a new machine)

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
export APPLE_API_KEY="<key-id>"                  # 10-char Key ID
export APPLE_API_ISSUER="<issuer-id>"            # account-wide UUID
export APPLE_API_KEY_PATH="$HOME/.secrets/AuthKey_<key-id>_appstoreconnect.p8"

npm run tauri:build:universal
```

The real Key ID and Issuer ID are kept **out of this repository** alongside the
`.p8` itself, in `~/.secrets/` (the `.p8` at mode `600`, in a `700` directory).
Neither authenticates anything without the key file, but the Issuer ID is
account-wide rather than per-project, so it stays out of a public repo. The Key
ID is also the filename of the `.p8`, so `ls ~/.secrets/` recovers it.

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
This covers the `.app` only — the DMG needs the extra step below before it can be
distributed.

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

### `tauri build` does not notarize the DMG

**Tauri notarizes and staples the `.app`, then builds the DMG around the already
stapled app and signs it — it never submits the DMG itself.** So a build that
looks entirely successful leaves the container in this state:

```
TimeBox.app                    accepted, source=Notarized Developer ID   ✅ stapled
TimeBox_0.1.0_universal.dmg    rejected, source=Unnotarized Developer ID ❌ no ticket
```

Observed on the first real Developer ID build, 2026-08-20. It matters because
the DMG is the artifact people download: the quarantine flag lands on *it*, and
Gatekeeper evaluates *it* before the app inside is ever reached. Attaching this
file to a release means a Gatekeeper block for every visitor, even though the
app within is correctly notarized.

Notarize the container as a second step, with the §3 credentials exported. No
rebuild — stapling attaches a ticket to the existing file and does not touch the
app inside it, so the app's own signature and ticket are unaffected:

```bash
DMG=src-tauri/target/universal-apple-darwin/release/bundle/dmg/TimeBox_0.1.0_universal.dmg

xcrun notarytool submit "$DMG" \
  --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" \
  --wait
xcrun stapler staple "$DMG"
```

`--wait` blocks until Apple returns a verdict, usually a few minutes. On
`status: Invalid`, `xcrun notarytool log <submission-id>` with the same
credentials gives the per-file reason.

### Verifying the result

```bash
./scripts/verify-release.sh
```

Seven checks — universal binary, signature validity, Developer ID authority,
hardened runtime, Team ID, Gatekeeper, and stapling **on both the app and the
DMG**. All of them run even when one fails, so one pass reports everything, and
the exit status is 0 only if every check passed. The version is read from
`tauri.conf.json`, so it does not go stale.

It distinguishes the two failure shapes it can see: only-the-DMG failing means
the build environment was right and the container just needs the step above,
while an app-level failure means the §3 environment was missing from the shell
that ran the build — in which case Tauri signs without notarizing, or skips
signing entirely, and only warns.

The individual commands, if you want them by hand:

```bash
APP=src-tauri/target/universal-apple-darwin/release/bundle/macos/TimeBox.app
codesign --verify --deep --strict --verbose=2 "$APP"
spctl -a -vvv -t install "$APP"          # expect: accepted, source=Notarized Developer ID
xcrun stapler validate "$APP"            # expect: The validate action worked!
xcrun stapler validate "$DMG"            # the one tauri build does not satisfy
spctl -a -vvv -t open --context context:primary-signature "$DMG"
```

The real test is a *different* Mac: copy the DMG across, open it, and confirm no
Gatekeeper warning. A build that passes `spctl` locally can still be quarantined
elsewhere if the ticket was not stapled — this machine can fall back to an
online check, and a Mac without network cannot.

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

## 6. Publishing — the download page

Two independent pieces. **The DMG you attach must be the notarized, stapled one
from §3** — an unsigned build behind a public download button is worse than no
download button, because every visitor gets Gatekeeper's "damaged and can't be
opened" — and `tauri build` does not produce a notarized DMG on its own. Run
`./scripts/verify-release.sh` and require a clean exit before the release leaves
draft.

### The release (what `Download for macOS` points at)

`docs/index.html` and the README link to
`https://github.com/gigiheristiawan/timebox/releases/latest`. That URL 404s
until one non-draft release exists; after that it always redirects to the newest
one, so the link never needs editing again.

```bash
# only after ./scripts/verify-release.sh exits clean, DMG checks included
gh release create v0.1.0 \
  src-tauri/target/universal-apple-darwin/release/bundle/dmg/TimeBox_0.1.0_universal.dmg \
  --title "TimeBox 0.1.0" \
  --notes-file docs/release-notes/0.1.0.md \
  --draft
```

Release notes live at `docs/release-notes/<version>.md`, one file per release,
so the published notes are reviewable in the repo before they are public.

The tag must match `version` in **both** `package.json` and
`src-tauri/tauri.conf.json` — they are the version the app reports, and a
mismatch means the About box disagrees with the download.

Use `--draft` to stage the assets and publish from the web UI once the download
has been verified on another Mac; a draft is invisible to `releases/latest`.

### The landing page (GitHub Pages)

Served from the `docs/` folder on `main` — no workflow, no build step, so a
`git push` publishes it.

```bash
gh api -X POST repos/gigiheristiawan/timebox/pages \
  -f 'source[branch]=main' -f 'source[path]=/docs'

# and point the repo header at it
gh repo edit gigiheristiawan/timebox \
  --homepage https://gigiheristiawan.github.io/timebox/ \
  --description "Task rotation timeboxing for macOS. The timer controls how long you work on a task, not whether it's done."
```

Live at `https://gigiheristiawan.github.io/timebox/`, first build ~1 minute.

Everything the page loads must live **under `docs/`** — the repo root is not the
site root. The icon it shows is vendored at `docs/assets/icon-256.png` for that
reason; a path reaching up into `src-tauri/icons/` renders locally and 404s once
published. Screenshots and the tray marks are already relative and fine.

---

## 7. Mac App Store

A second distribution channel, alongside the Developer ID DMG of §2–§3 — not a
replacement. Same source, different packaging: the App Store build is
**sandboxed**, so its database lives in
`~/Library/Containers/xyz.gigiheristiawan.timebox/Data/Library/Application Support/`
and an existing DMG user's data is invisible to it. That is accepted for now
(0.1.0, few users) and must be said in the store description.

```bash
./scripts/build-mas.sh
```

Everything below is what that script does and why, plus the one-time account
setup it cannot do for you.

### 7.1 What had to change in the source

Three things made the app un-shippable to the store; all three are fixed in
`main`, so the DMG build and the store build come from the same tree.

| Was | Why it fails review or the sandbox | Now |
| --- | --- | --- |
| `tauri` feature `macos-private-api`, `popover.transparent(true)` | Reaches the WKWebView background through a private key. Private API is an automatic rejection. | `platform/window_corners.rs` — `setOpaque:NO`, clear window colour, and a corner radius on the content view's layer. All public AppKit, same rounded card. |
| `tauri-plugin-autostart` (`MacosLauncher::LaunchAgent`) | Writes `~/Library/LaunchAgents/…`, outside the container. The sandbox denies it and the toggle silently does nothing. | `platform/login_item.rs` — `SMAppService.mainApp` (macOS 13+, matching `minimumSystemVersion`). |
| `tauri-plugin-single-instance` reopening the popover | Binds a Unix socket at `/tmp/…`, which the sandbox denies. | Still present for the DMG build; `RunEvent::Reopen` in `lib.rs` covers the same behaviour and is what a store-installed bundle actually gets, since Launch Services reopens a bundle rather than starting a second copy. |

`SMAppService.mainApp` **raises an Objective-C exception when the process is not
in a bundle** — and Rust cannot catch it, so the app aborts. `login_item.rs`
guards on `NSBundle.mainBundle.bundleIdentifier` for that reason: under
`tauri:dev` the toggle logs a refusal instead of killing the app. Do not remove
the guard, and do not expect launch-at-login to work in a dev build.

`SMAppService` can also **refuse**, which the plist it replaced never could: it
wants the app signed and installed in `/Applications`. `login_item::reconcile`
therefore runs on every launch, not only when the setting is edited, and the
snapshot carries `launchAtLoginActive` so the settings toggle reports what macOS
actually did. Expect the toggle to show its refusal note in any build running
from `target/` — that is correct behaviour, not a bug.

**Upgrading from 0.1.0 leaves a booby trap.** That build used
`tauri-plugin-autostart`, which wrote `~/Library/LaunchAgents/TimeBox.plist`
with `RunAtLoad`. The new code registers through `SMAppService` and has no
knowledge of that file, so without cleanup a user would have two independent
launch-at-login mechanisms — and turning the setting *off* would unregister the
service while the plist kept starting the app. `login_item::remove_legacy_launch_agent()`
runs at startup to delete it, gated on the plist actually pointing at a
`TimeBox.app`. Sandboxed builds no-op there, which is correct: a store install
never had the file.

`Info.plist` gained `ITSAppUsesNonExemptEncryption=false` — without it App Store
Connect asks the export-compliance question on every single upload.

### 7.2 One-time account setup

None of this is scriptable; do it once at developer.apple.com and App Store
Connect.

1. **Register the bundle ID** `xyz.gigiheristiawan.timebox` (Identifiers → App
   IDs → macOS). It is not registered by the Developer ID build — that one never
   needed an App ID.
2. **Two new certificates**, neither of which is the Developer ID Application
   cert §3 talks about:
   - `3rd Party Mac Developer Application` — signs the .app
   - `3rd Party Mac Developer Installer` — signs the .pkg
3. **A Mac App Store provisioning profile** for that App ID, downloaded to
   `~/.secrets/timebox_mas.provisionprofile` (or point `PROVISION_PROFILE` at it).
   The script embeds it as `Contents/embedded.provisionprofile`; without it the
   store rejects the upload and the installed app refuses to launch.
4. **A new app record** in App Store Connect: category (Productivity),
   description, screenshots (1280×800 or 1440×900), and a privacy label — TimeBox
   collects nothing, which is the shortest form of that questionnaire.

`entitlements.mas.plist` carries `TEAM_ID_PLACEHOLDER` rather than the real Team
ID; `build-mas.sh` substitutes `$TEAM_ID` at signing time and then verifies the
substituted `application-identifier` matches the bundle identifier, so a
mismatched profile fails locally instead of after a 20-minute upload.

### 7.3 Verifying before upload

`scripts/verify-release.sh` checks notarization and stapling — **neither applies
here**. App Store builds are not notarized by you; the store does its own
processing. What matters instead:

```bash
codesign -d --entitlements - --xml target-mas/TimeBox.app | grep app-sandbox
lipo -archs target-mas/TimeBox.app/Contents/MacOS/TimeBox   # x86_64 arm64
```

The script asserts both.

### The .pkg cannot be run locally — do not try

A Mac App Store build launches only where the Mac App Store installed it.
Installing the .pkg by hand and opening the app fails:

```
open /Applications/TimeBox.app
# Launch failed. … NSPOSIXErrorDomain Code=163 "Launchd job spawn failed"
```

Nothing is wrong with the build when that happens. The signature verifies and
satisfies its Designated Requirement; what fails is authorisation:

```bash
spctl -a -vvv -t exec /Applications/TimeBox.app     # rejected
```

Gatekeeper accepts Developer ID + notarized, or apps the store installed —
a `3rd Party Mac Developer Application` signature is neither. And a Mac App
Store provisioning profile carries no `ProvisionedDevices`, so no Mac is
authorised. The .pkg is an **upload artifact, not an installable build**.

### Local sandbox verification instead

```bash
./scripts/sandbox-smoketest.sh
```

It re-signs the same bundle with the **Developer ID** identity and the sandbox
entitlement alone, which does run locally. That covers what actually risks a
rejection — container creation, SQLite, notifications, the global shortcut, and
launch-at-login through `SMAppService`, which no dev build can test because an
unbundled binary is refused. It does not cover the MAS provisioning path; use
**TestFlight for macOS** after uploading for that.

### The installer will hide your app somewhere else

`productbuild --component` marks the bundle relocatable, so Installer searches
the disk for an existing bundle with the same `CFBundleIdentifier` and installs
over *that* instead of `/Applications`:

```
PackageKit: Applications/TimeBox.app relocated to
  …/src-tauri/target/universal-apple-darwin/release/bundle/macos/TimeBox.app
```

Any previous build output is a candidate, so this fires almost every time. It
does not affect real store users — the Mac App Store installs apps directly,
never through Installer.app. If you install the .pkg by hand anyway, delete the
other bundles first, and note that a relocated install leaves them **root-owned**,
so a second attempt needs `sudo rm -rf`. Check where it actually went with:

```bash
grep -i relocated /var/log/install.log | tail -2
```

### 7.4 Review risks worth pre-empting

- **The checkpoint has no exit** (SPEC D14) — that is the product, not a bug, but
  a reviewer will try to dismiss it. `Cmd+Q` and the quit-confirm still work, so
  the app is never unquittable. Say so in the review notes.
- **Menu bar only** (`LSUIElement`) — reviewers have opened a rejection on "the
  app does nothing when I launch it" before. Review notes should say the icon is
  in the menu bar and `Cmd+Shift+T` opens the popover.
- **1-minute `TEMPORARY` durations** — `grep -r TEMPORARY src/` must be empty.
