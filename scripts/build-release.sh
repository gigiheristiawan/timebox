#!/usr/bin/env bash
# Build, notarize, verify and stage the Developer ID / GitHub release.
#
# This is RELEASE.md §3–§6 in one pass: the universal signed build, the DMG
# notarization Tauri does not do (it notarizes the .app and then builds the
# container around it), the staple, ./scripts/verify-release.sh, and the draft
# GitHub release. The counterpart to build-mas.sh, which packages the same
# source for the App Store; the two builds differ in signature, sandbox and
# container and neither can stand in for the other.
#
# Not covered, because they are judgement and not mechanism (RELEASE.md §0):
#   1. the version bump — five files, CFBundleVersion moving independently
#   2. docs/release-notes/<version>.md
# Both are *checked* here, and the run stops if either is missing.
#
# The release is left as a DRAFT. Publishing is manual and deliberate: a draft
# is invisible to releases/latest, which is what the website's download button
# points at, and the DMG should be opened on a different Mac first.
#
# Usage:
#   ./scripts/build-release.sh              # build → notarize → verify → draft
#   ./scripts/build-release.sh --skip-build # reuse the bundle already on disk
#   ./scripts/build-release.sh --no-release # stop after verification
set -euo pipefail

cd "$(dirname "$0")/.."

SKIP_BUILD=0
NO_RELEASE=0
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=1 ;;
    --no-release) NO_RELEASE=1 ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

bold=$'\033[1m'; red=$'\033[31m'; dim=$'\033[2m'; off=$'\033[0m'
fail() { echo "${red}error:${off} $*" >&2; exit 1; }
step() { printf '\n%s\n' "${bold}==> $*${off}"; }

# The Key ID and Issuer ID are account-wide secrets and stay out of the repo
# (RELEASE.md §3). Keep them in this file — mode 600, in a 700 directory —
# and the script needs no arguments; or export them yourself beforehand.
ENV_FILE="${TIMEBOX_RELEASE_ENV:-$HOME/.secrets/timebox-release.env}"
if [ -f "$ENV_FILE" ]; then
  # shellcheck disable=SC1090
  . "$ENV_FILE"
fi

: "${APPLE_SIGNING_IDENTITY:=Developer ID Application: Gigih Eristiawan (SQ3B3PDL4S)}"
: "${APPLE_API_KEY_PATH:=$HOME/.secrets/AuthKey_${APPLE_API_KEY:-}_appstoreconnect.p8}"
export APPLE_SIGNING_IDENTITY APPLE_API_KEY_PATH

# Tauri picks the Apple ID credentials over the API key when both are present,
# and a stale APPLE_ID in the shell then makes the build ask for a password it
# will never get. The API key is the only credential this path uses.
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID

# ------------------------------------------------------------------ preflight

VERSION=$(python3 -c 'import json;print(json.load(open("src-tauri/tauri.conf.json"))["version"])')
PKG_VERSION=$(python3 -c 'import json;print(json.load(open("package.json"))["version"])')
CARGO_VERSION=$(sed -n '/^\[package\]/,/^\[/s/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)
NOTES="docs/release-notes/$VERSION.md"
BUNDLE="src-tauri/target/universal-apple-darwin/release/bundle"
APP="$BUNDLE/macos/TimeBox.app"
DMG="$BUNDLE/dmg/TimeBox_${VERSION}_universal.dmg"
TAG="v$VERSION"

printf '%s\n' "${bold}TimeBox $VERSION — Developer ID release${off}"
printf '%s\n' "${dim}signing as: $APPLE_SIGNING_IDENTITY${off}"

step "preflight"

# The tag names the version the app reports; a disagreement ships a build whose
# Settings pane contradicts its own download link (RELEASE.md §0, §6).
[ "$PKG_VERSION" = "$VERSION" ] \
  || fail "package.json says $PKG_VERSION, tauri.conf.json says $VERSION"
[ "$CARGO_VERSION" = "$VERSION" ] \
  || fail "src-tauri/Cargo.toml says $CARGO_VERSION, tauri.conf.json says $VERSION"
LOCK_VERSION=$(awk '/^name = "timebox"$/{getline; gsub(/[^0-9.]/, "", $0); print; exit}' src-tauri/Cargo.lock)
[ "$LOCK_VERSION" = "$VERSION" ] \
  || fail "src-tauri/Cargo.lock says ${LOCK_VERSION:-nothing} — cargo would rewrite it mid-release"

[ -f "$NOTES" ] || fail "no release notes at $NOTES
Write them before the build, not after (RELEASE.md §0): it is the step that
catches a reversed invariant while there is still time to reconsider."

command -v gh >/dev/null || fail "gh is not installed"
[ "$NO_RELEASE" = 1 ] || gh auth status >/dev/null 2>&1 || fail "gh is not authenticated"

[ -n "${APPLE_API_KEY:-}" ] || fail "APPLE_API_KEY (the 10-char Key ID) is not set.
Put it, APPLE_API_ISSUER and optionally APPLE_SIGNING_IDENTITY in $ENV_FILE."
[ -n "${APPLE_API_ISSUER:-}" ] || fail "APPLE_API_ISSUER (the account-wide UUID) is not set"
[ -f "$APPLE_API_KEY_PATH" ] || fail "no App Store Connect key at $APPLE_API_KEY_PATH"
security find-identity -v -p codesigning | grep -qF "$APPLE_SIGNING_IDENTITY" \
  || fail "no signing identity: $APPLE_SIGNING_IDENTITY"

# `gh release create` tags the current commit, so an uncommitted bump would tag
# a tree that still claims the old version.
if [ -n "$(git status --porcelain)" ]; then
  echo "${red}The working tree is dirty:${off}"
  git status --short
  fail "commit the version bump and the notes first — the tag points at HEAD"
fi
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null \
   || gh release view "$TAG" >/dev/null 2>&1; then
  fail "$TAG already exists. Bump the version, or delete the draft release and the tag."
fi

echo "  version    $VERSION (tauri.conf.json, package.json, Cargo.toml agree)"
echo "  notes      $NOTES"
echo "  tag        $TAG at $(git rev-parse --short HEAD)"

# ---------------------------------------------------------------------- build

if [ "$SKIP_BUILD" = 1 ]; then
  step "skipping build (--skip-build)"
  [ -f "$DMG" ] || fail "no DMG at $DMG — there is nothing to reuse"
else
  step "building signed + notarized universal bundle"
  npm run tauri:build:universal
fi

[ -f "$DMG" ] || fail "expected $DMG, which the bundler did not produce"

# --------------------------------------------------------- staple the .app

# Tauri notarizes the .app and staples it, then builds the DMG around the
# result. The staple often fails there: `--wait` returns when Apple's *verdict*
# is Accepted, but the ticket `stapler` fetches is published to Apple's
# distribution servers a little later, and Tauri treats the failure as a
# warning. What comes out is an app that is notarized but carries no proof of
# it, sealed inside a DMG that was built a moment later.
#
# So: staple it here, retrying for as long as the delay usually lasts.
if xcrun stapler validate "$APP" >/dev/null 2>&1; then
  echo "  the .app is already stapled"
else
  step "stapling the .app (Tauri's staple did not take)"
  stapled=0
  for attempt in 1 2 3 4 5; do
    if xcrun stapler staple "$APP"; then stapled=1; break; fi
    echo "  attempt $attempt: the ticket is not published yet — retrying in 30s"
    sleep 30
  done
  [ "$stapled" = 1 ] || fail "the app could not be stapled after five attempts.
If it was never notarized, the §3 credentials were missing from the shell that
ran the build — Tauri warns and carries on without them. Check with:
  spctl -a -vvv -t install \"$APP\""
fi

# Whether the DMG has to be rebuilt is decided by the copy *inside* it, never
# by the loose bundle: stapling the one on disk does not reach the one already
# sealed in the container, and after a hand-run `stapler staple` the two
# disagree. Mount it and ask.
MNT="$BUNDLE/dmg/mnt-repack"
rm -rf "$MNT" && mkdir -p "$MNT"
hdiutil attach "$DMG" -nobrowse -readonly -mountpoint "$MNT" -quiet
if xcrun stapler validate "$MNT/TimeBox.app" >/dev/null 2>&1; then
  REPACK=0
  echo "  the app inside the DMG is stapled"
else
  REPACK=1
  echo "  the app inside the DMG has no ticket — the DMG will be repacked"
fi
hdiutil detach "$MNT" -quiet
rmdir "$MNT"

if [ "$REPACK" = 1 ]; then
  step "repacking the DMG around the stapled app"
  # Cheaper and more faithful than rebuilding: the DMG's contents, layout,
  # volume name and the /Applications symlink are all preserved, and the app
  # inside is the same signed bundle — it only gains its ticket. Converting
  # away from the compressed format is the only way to write into it.
  RW="$BUNDLE/dmg/rw-repack.dmg"
  cleanup() {
    hdiutil detach "$MNT" -force >/dev/null 2>&1 || true
    rm -rf "$RW" "$MNT"
  }
  trap cleanup EXIT

  rm -rf "$RW" "$MNT" && mkdir -p "$MNT"
  hdiutil convert "$DMG" -format UDRW -o "$RW" -quiet
  hdiutil attach "$RW" -nobrowse -mountpoint "$MNT" -quiet
  xcrun stapler staple "$MNT/TimeBox.app"
  hdiutil detach "$MNT" -quiet
  hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -o "$DMG" -ov -quiet

  cleanup
  trap - EXIT

  # The container was rewritten, so whatever ticket and signature it held are
  # void. Both are restored below, in that order.
  RENOTARIZE=1
fi

# ----------------------------------------------------- sign + notarize DMG

# Tauri signs the DMG it builds. `hdiutil convert` writes a new file and does
# not carry that signature over, so a repacked container has none — Gatekeeper
# then rejects it with `source=no usable signature`, whatever its ticket says.
if ! codesign -dv "$DMG" >/dev/null 2>&1; then
  step "signing the DMG"
  codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$DMG"
  # A signature rewrites the file, so any ticket it held describes content that
  # no longer exists. Notarization has to happen after this, never before.
  RENOTARIZE=1
fi

# Tauri submits the .app and staples it, then builds the DMG around the result
# — so the container itself is unnotarized and Gatekeeper judges the file the
# user actually downloads (RELEASE.md §3, observed on the first 0.1.0 build).
# Stapling attaches a ticket to the existing file, so this is safe to re-run.
# The skip needs both halves: a ticket alone is not a verdict Gatekeeper will
# give, and a DMG can be stapled and unsigned at once.
if [ "${RENOTARIZE:-0}" = 0 ] \
   && xcrun stapler validate "$DMG" >/dev/null 2>&1 \
   && spctl -a -t open --context context:primary-signature "$DMG" >/dev/null 2>&1; then
  step "DMG already signed, notarized and stapled — skipping"
else
  step "notarizing the DMG"
  xcrun notarytool submit "$DMG" \
    --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" \
    --wait

  step "stapling the ticket"
  xcrun stapler staple "$DMG"
fi

# -------------------------------------------------------------------- verify

step "verifying"
# Every check inside runs even when one fails, and its exit status is 0 only if
# all of them passed — so this is the gate, not an advisory.
./scripts/verify-release.sh || fail "verification failed — do not release this DMG"

# ------------------------------------------------------------------- release

if [ "$NO_RELEASE" = 1 ]; then
  step "stopping before the release (--no-release)"
  echo "Verified artifact: $DMG"
  exit 0
fi

step "creating the draft release"
gh release create "$TAG" "$DMG" \
  --title "TimeBox $VERSION" \
  --notes-file "$NOTES" \
  --draft

cat <<MSG

${bold}Draft release $TAG created.${off}

  gh release view $TAG --web

Before publishing, copy the DMG to a different Mac, open it, and confirm there
is no Gatekeeper warning — local checks cannot produce that verdict, because
this machine can fall back to an online notarization check.

Publish from the web UI. A draft is invisible to releases/latest, which is what
the site's ${dim}Download for macOS${off} button follows.
MSG
