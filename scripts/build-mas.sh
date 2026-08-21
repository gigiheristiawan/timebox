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
set -euo pipefail

cd "$(dirname "$0")/.."

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

cat <<MSG

Built $PKG

Upload with either:
  xcrun altool --upload-app -f "$PKG" -t macos \\
    --apiKey <key-id> --apiIssuer <issuer-id>
  # or the Transporter app from the Mac App Store

Note: this .pkg is an upload artifact, NOT an installable build. Installing it
by hand and launching the app fails ("Launchd job spawn failed"): Gatekeeper
rejects a 3rd Party Mac Developer signature, and a Mac App Store provisioning
profile authorises no devices. Nothing is wrong with the build when that happens.

To verify the sandbox locally, use ./scripts/sandbox-smoketest.sh, which
re-signs the same bundle with Developer ID + the sandbox entitlement. For the
real Mac App Store path, use TestFlight for macOS after uploading.
MSG
