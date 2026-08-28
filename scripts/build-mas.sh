#!/usr/bin/env bash
# Build a Mac App Store submission package.
#
# Tauri has no App Store target: it produces a .app, and everything the store
# needs — the sandbox entitlements, the embedded provisioning profile, the
# Apple Distribution signature and the signed installer .pkg — is applied here,
# after the bundle exists.
#
# This does NOT replace the Developer ID path in docs/RELEASE.md §3. That build
# stays unsandboxed so existing DMG users keep their database.
#
# Usage:
#   ./scripts/build-mas.sh                     # build the .pkg only
#   ./scripts/build-mas.sh --validate          # build, then validate with App Store Connect
#   ./scripts/build-mas.sh --validate --upload # build, validate, then upload for TestFlight
#   ./scripts/build-mas.sh --upload --skip-build   # upload the .pkg already staged
#
# --validate is the cheap half of an upload: App Store Connect runs the same
# entitlement, icon and version checks and returns them in seconds, where a
# failed upload reports them by email ~20 minutes later. Always run it first.
# --upload delivers the build; it does not submit it for review.
set -euo pipefail

cd "$(dirname "$0")/.."

VALIDATE=0
UPLOAD=0
SKIP_BUILD=0
for arg in "$@"; do
  case "$arg" in
    --validate) VALIDATE=1 ;;
    --upload) UPLOAD=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    -h|--help) sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

: "${TEAM_ID:=SQ3B3PDL4S}"
: "${PROVISION_PROFILE:=$HOME/.secrets/timebox_mas.provisionprofile}"
: "${APP_CERT:=3rd Party Mac Developer Application: Gigih Eristiawan ($TEAM_ID)}"
: "${INSTALLER_CERT:=3rd Party Mac Developer Installer: Gigih Eristiawan ($TEAM_ID)}"

BUNDLE_ID=$(python3 -c 'import json;print(json.load(open("src-tauri/tauri.conf.json"))["identifier"])')
VERSION=$(python3 -c 'import json;print(json.load(open("src-tauri/tauri.conf.json"))["version"])')
BUILD_DIR=src-tauri/target/universal-apple-darwin/release/bundle/macos
STAGE=target-mas
APP="$STAGE/TimeBox.app"
PKG="$STAGE/TimeBox_${VERSION}_mas.pkg"

fail() { echo "error: $*" >&2; exit 1; }

# The App Store Connect API key. Same credentials the Developer ID release
# uses, and kept the same way: the Key ID and the account-wide Issuer ID are
# secrets, so they live outside the repo (RELEASE.md §3).
if [ "$VALIDATE" = 1 ] || [ "$UPLOAD" = 1 ]; then
  ENV_FILE="${TIMEBOX_RELEASE_ENV:-$HOME/.secrets/timebox-release.env}"
  # shellcheck disable=SC1090
  [ -f "$ENV_FILE" ] && . "$ENV_FILE"
  : "${APPLE_API_KEY_PATH:=$HOME/.secrets/AuthKey_${APPLE_API_KEY:-}_appstoreconnect.p8}"

  [ -n "${APPLE_API_KEY:-}" ] || fail "APPLE_API_KEY (the 10-char Key ID) is not set.
Put it and APPLE_API_ISSUER in $ENV_FILE, or export them."
  [ -n "${APPLE_API_ISSUER:-}" ] || fail "APPLE_API_ISSUER (the account-wide UUID) is not set"
  [ -f "$APPLE_API_KEY_PATH" ] || fail "no App Store Connect key at $APPLE_API_KEY_PATH"

  # altool only looks for the .p8 in a fixed set of directories, none of which
  # is ~/.secrets. This is the documented way to point it elsewhere, and it
  # avoids a second copy of the key sitting in ~/.private_keys.
  export API_PRIVATE_KEYS_DIR="$(dirname "$APPLE_API_KEY_PATH")"
fi

if [ "$SKIP_BUILD" = 1 ]; then
  [ -f "$PKG" ] || fail "no package at $PKG — there is nothing to reuse"
  echo "==> reusing $PKG (--skip-build)"
else

[ -f "$PROVISION_PROFILE" ] || fail "no provisioning profile at $PROVISION_PROFILE
Create a Mac App Store profile for $BUNDLE_ID at developer.apple.com and download it there."
security find-identity -v -p codesigning | grep -qF "$APP_CERT" \
  || fail "no signing identity: $APP_CERT"

echo "==> building universal .app"
npm run tauri -- build --target universal-apple-darwin --bundles app

rm -rf "$STAGE" && mkdir -p "$STAGE"
cp -R "$BUILD_DIR/TimeBox.app" "$APP"

echo "==> embedding provisioning profile"
cp "$PROVISION_PROFILE" "$APP/Contents/embedded.provisionprofile"

# The entitlements in the repo carry a placeholder so the Team ID is not
# duplicated across files; the real one is substituted at signing time.
ENTITLEMENTS="$STAGE/entitlements.plist"
sed "s/TEAM_ID_PLACEHOLDER/$TEAM_ID/g" src-tauri/entitlements.mas.plist > "$ENTITLEMENTS"
grep -q "$TEAM_ID.$BUNDLE_ID" "$ENTITLEMENTS" \
  || fail "entitlements application-identifier does not match $BUNDLE_ID"

# Anything downloaded through a browser carries com.apple.quarantine, and the
# provisioning profile always is. A single quarantined file rejects the whole
# submission (ITMS-91109) — after upload, by email, ~20 minutes later. Strip
# every extended attribute from the staged copy.
#
# This must happen BEFORE signing: codesign stores signatures for some file
# types in extended attributes, so clearing them afterwards would quietly
# invalidate the very signature it just wrote.
echo "==> clearing extended attributes"
xattr -cr "$APP"

echo "==> signing"
# Nested code first: a signature over the bundle is only valid if what it
# contains was already sealed.
find "$APP/Contents" \( -name '*.dylib' -o -name '*.framework' \) -print0 \
  | xargs -0 -r codesign --force --timestamp --options runtime --sign "$APP_CERT"
codesign --force --timestamp --options runtime \
  --entitlements "$ENTITLEMENTS" --sign "$APP_CERT" "$APP"

codesign --verify --strict --verbose=2 "$APP"
codesign -d --entitlements - --xml "$APP" | grep -q app-sandbox \
  || fail "the signed app is not sandboxed"
if xattr -lr "$APP" | grep -q com.apple.quarantine; then
  fail "a quarantined file survived in $APP — Apple rejects these (ITMS-91109)"
fi

echo "==> packaging"
productbuild --component "$APP" /Applications --sign "$INSTALLER_CERT" "$PKG"

fi  # end of the build

# The build number, not the marketing version, is what App Store Connect keys a
# delivery on: it must strictly increase on every upload, a failed one included.
BUILD_NO=$(defaults read "$PWD/$APP/Contents/Info" CFBundleVersion 2>/dev/null || echo "?")

if [ "$VALIDATE" = 1 ]; then
  echo "==> validating with App Store Connect"
  xcrun altool --validate-app -f "$PKG" -t macos \
    --apiKey "$APPLE_API_KEY" --apiIssuer "$APPLE_API_ISSUER" \
    || fail "validation failed — do not upload this package"
fi

if [ "$UPLOAD" = 1 ]; then
  [ "$VALIDATE" = 1 ] || echo "note: uploading without --validate; failures come back by email, not now"
  echo "==> uploading $VERSION (build $BUILD_NO)"
  xcrun altool --upload-app -f "$PKG" -t macos \
    --apiKey "$APPLE_API_KEY" --apiIssuer "$APPLE_API_ISSUER"
  cat <<MSG

Uploaded $VERSION (build $BUILD_NO).

Processing takes ~10-30 minutes; App Store Connect emails the result. Until it
finishes the build does not appear in TestFlight. A rejection at this stage is
reported by email only — nothing above will have failed.

The next upload needs a higher CFBundleVersion in src-tauri/Info.plist, even if
this one is rejected: the delivery is registered either way.
MSG
  exit 0
fi

cat <<MSG

Built $PKG

Validate and upload with:
  ./scripts/build-mas.sh --validate --upload --skip-build

or by hand, with the API key exported:
  xcrun altool --validate-app -f "$PKG" -t macos \\
    --apiKey "\$APPLE_API_KEY" --apiIssuer "\$APPLE_API_ISSUER"
  xcrun altool --upload-app -f "$PKG" -t macos \\
    --apiKey "\$APPLE_API_KEY" --apiIssuer "\$APPLE_API_ISSUER"
  # or the Transporter app from the Mac App Store

Note: this .pkg is an upload artifact, NOT an installable build. Installing it
by hand and launching the app fails ("Launchd job spawn failed"): Gatekeeper
rejects a 3rd Party Mac Developer signature, and a Mac App Store provisioning
profile authorises no devices. Nothing is wrong with the build when that happens.

To verify the sandbox locally, use ./scripts/sandbox-smoketest.sh, which
re-signs the same bundle with Developer ID + the sandbox entitlement. For the
real Mac App Store path, use TestFlight for macOS after uploading.
MSG
