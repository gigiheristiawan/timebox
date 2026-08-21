#!/usr/bin/env bash
# Run TimeBox sandboxed, locally.
#
# A Mac App Store build cannot be launched on your own Mac: it is signed with a
# 3rd Party Mac Developer certificate that Gatekeeper does not accept, and its
# provisioning profile authorises no devices. `open` fails with a launchd spawn
# error. The .pkg from build-mas.sh is an upload artifact, not an installable
# build — see docs/RELEASE.md §7.3.
#
# So this script re-signs the same bundle with your **Developer ID** identity
# and the sandbox entitlement alone. That runs locally, and it exercises the
# thing worth testing before submission: whether TimeBox actually works with
# the App Sandbox on — container creation, SQLite, notifications, the global
# shortcut, and launch-at-login through SMAppService.
#
# What it does NOT test is the Mac App Store provisioning plumbing. Nothing
# local can; use TestFlight for macOS after uploading for that.
#
# The artifact this produces is deliberately not submittable. Do not ship it.
set -euo pipefail

cd "$(dirname "$0")/.."

: "${TEAM_ID:=SQ3B3PDL4S}"
: "${SIGN_CERT:=Developer ID Application: Gigih Eristiawan ($TEAM_ID)}"

STAGE=target-sandbox-test
APP="$STAGE/TimeBox.app"
INSTALLED=/Applications/TimeBox.app
BUNDLE_ID=$(python3 -c 'import json;print(json.load(open("src-tauri/tauri.conf.json"))["identifier"])')
CONTAINER="$HOME/Library/Containers/$BUNDLE_ID"

fail() { echo "error: $*" >&2; exit 1; }

# Pick the NEWEST built bundle, never the first one found.
#
# This script re-signs; it does not compile. So the one risk that matters is
# re-signing a stale bundle and testing a fix that is not in it — which is
# exactly what happened once: a leftover target-mas/ staging copy shadowed a
# fresh `tauri:build`, and the crash under test reappeared unchanged. Ordering
# by mtime plus the staleness check below makes that failure impossible rather
# than merely unlikely.
SRC=""
for c in src-tauri/target/release/bundle/macos/TimeBox.app \
         src-tauri/target/universal-apple-darwin/release/bundle/macos/TimeBox.app \
         target-mas/TimeBox.app target-mas/TimeBox.app.staged; do
  [ -d "$c" ] || continue
  if [ -z "$SRC" ] || [ "$c/Contents/MacOS/timebox" -nt "$SRC/Contents/MacOS/timebox" ]; then
    SRC="$c"
  fi
done
[ -n "$SRC" ] || fail "no built TimeBox.app found.
Run: npm run tauri:build"

# Refuse to test a bundle older than the source it claims to be built from.
NEWEST_SRC=$(find src-tauri/src src -type f \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' \) \
             -newer "$SRC/Contents/MacOS/timebox" -print -quit 2>/dev/null || true)
[ -z "$NEWEST_SRC" ] || fail "$SRC is older than $NEWEST_SRC
The binary predates your source changes. Run: npm run tauri:build"

echo "==> using $SRC (built $(stat -f '%Sm' -t '%Y-%m-%d %H:%M:%S' "$SRC/Contents/MacOS/timebox"))"

security find-identity -v -p codesigning | grep -qF "$SIGN_CERT" \
  || fail "no signing identity: $SIGN_CERT"

echo "==> staging from $SRC"
rm -rf "$STAGE" && mkdir -p "$STAGE"
cp -R "$SRC" "$APP"

# The Mac App Store profile means nothing to a Developer ID signature, and
# leaving it in place only invites confusion about which build this is.
rm -f "$APP/Contents/embedded.provisionprofile"

# Sandbox only. `application-identifier` and `team-identifier` are omitted
# deliberately: they require a provisioning profile, and adding them without
# one makes the signature unusable.
ENTITLEMENTS="$STAGE/entitlements.plist"
cat > "$ENTITLEMENTS" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <true/>
    <!-- WKWebView will not spawn its WebContent process without this. -->
    <key>com.apple.security.network.client</key>
    <true/>
</dict>
</plist>
PLIST

echo "==> signing with $SIGN_CERT"
find "$APP/Contents" \( -name '*.dylib' -o -name '*.framework' \) -print0 \
  | xargs -0 -r codesign --force --timestamp --options runtime --sign "$SIGN_CERT"
codesign --force --timestamp --options runtime \
  --entitlements "$ENTITLEMENTS" --sign "$SIGN_CERT" "$APP"

codesign --verify --strict --verbose=2 "$APP"
codesign -d --entitlements - --xml "$APP" | grep -q app-sandbox \
  || fail "the signed app is not sandboxed"

echo "==> installing to $INSTALLED"
# The pkg install leaves a root-owned bundle behind, hence sudo.
[ -e "$INSTALLED" ] && sudo rm -rf "$INSTALLED"
cp -R "$APP" "$INSTALLED"

cat <<MSG

Installed a sandboxed Developer ID build at $INSTALLED

Next:
  open $INSTALLED

Then report whether the tray icon appears and stays. The rest of the
verification (container, database, sandbox denials, launch-at-login) is
checked one step at a time from there — see docs/RELEASE.md §7.3.
MSG
