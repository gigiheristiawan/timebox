#!/usr/bin/env bash
#
# Verify a signed, notarized, stapled universal build before it is attached to a
# GitHub release. RELEASE.md §3 "Verifying the result" and §6.
#
# Run from the project root, after `npm run tauri:build:universal`:
#
#     ./scripts/verify-release.sh
#
# Every check runs even if an earlier one fails, so one pass tells you
# everything that is wrong. Exit status is 0 only if all of them passed.

set -uo pipefail

BUNDLE="src-tauri/target/universal-apple-darwin/release/bundle"
APP="$BUNDLE/macos/TimeBox.app"
BIN="$APP/Contents/MacOS/TimeBox"
TEAM_ID="SQ3B3PDL4S"

bold=$'\033[1m'; red=$'\033[31m'; green=$'\033[32m'; dim=$'\033[2m'; off=$'\033[0m'
failures=()

check() { # check <name> <command...>
  local name="$1"; shift
  printf '%s\n' "${bold}▸ ${name}${off}"
  if "$@" 2>&1 | sed 's/^/  /'; then
    printf '%s\n\n' "  ${green}PASS${off}"
  else
    printf '%s\n\n' "  ${red}FAIL${off}"
    failures+=("$name")
  fi
}

[ -f package.json ] || { echo "${red}Run this from the project root.${off}"; exit 2; }

VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
DMG="$BUNDLE/dmg/TimeBox_${VERSION}_universal.dmg"

printf '%s\n' "${bold}TimeBox ${VERSION} — release verification${off}"
printf '%s\n\n' "${dim}${APP}${off}"

# 0. Artifacts exist. Without these the rest is noise, so bail immediately.
missing=0
[ -d "$APP" ] || { echo "${red}Missing: $APP${off}"; missing=1; }
if [ ! -f "$DMG" ]; then
  # The bundler may have named it differently; take the only .dmg if there is one.
  found=$(find "$BUNDLE/dmg" -maxdepth 1 -name '*.dmg' 2>/dev/null)
  if [ "$(printf '%s' "$found" | grep -c .)" = 1 ]; then
    DMG="$found"
    echo "${dim}Using DMG: $DMG${off}"
  else
    echo "${red}Missing: $DMG${off}"
    [ -n "$found" ] && echo "${dim}Candidates:${off}" && echo "$found"
    missing=1
  fi
fi
if [ "$missing" = 1 ]; then
  echo
  echo "Build first:  npm run tauri:build:universal"
  echo "with the signing and notarization environment from RELEASE.md §3."
  exit 2
fi
echo

# 1. The universal binary survived signing.
check "Universal binary (expect: x86_64 arm64)" \
  bash -c 'archs=$(lipo -archs "$0"); echo "$archs"; [ "$archs" = "x86_64 arm64" ] || [ "$archs" = "arm64 x86_64" ]' "$BIN"

# 2. Signature is valid and self-consistent.
check "Signature valid (codesign --verify --deep --strict)" \
  codesign --verify --deep --strict --verbose=2 "$APP"

# 3. Signed by Developer ID, not the Development certificate, with the hardened
#    runtime on — notarization requires the runtime flag, and Gatekeeper
#    requires the Developer ID authority.
check "Developer ID authority + hardened runtime + Team ID" \
  bash -c '
    out=$(codesign -dvvv "$0" 2>&1)
    echo "$out" | grep -E "^Authority=|^TeamIdentifier=|flags=" || true
    ok=0
    echo "$out" | grep -q "Authority=Developer ID Application" || { echo "  ✗ not signed with a Developer ID Application certificate"; ok=1; }
    echo "$out" | grep -q "flags=.*runtime"                    || { echo "  ✗ hardened runtime is OFF — notarization will reject this"; ok=1; }
    echo "$out" | grep -q "TeamIdentifier=$1"                  || { echo "  ✗ Team ID is not $1"; ok=1; }
    exit $ok' "$APP" "$TEAM_ID"

# 4. Gatekeeper accepts the app as a notarized Developer ID build. This is the
#    line the unsigned/Development smoke test could never produce.
check "Gatekeeper accepts the app (expect: source=Notarized Developer ID)" \
  bash -c 'out=$(spctl -a -vvv -t install "$0" 2>&1); echo "$out"; echo "$out" | grep -q "Notarized Developer ID"' "$APP"

# 5. The ticket is stapled to both artifacts. Stapling is what makes the first
#    launch work on a Mac with no network — spctl passing locally does not
#    imply it, because this machine can fall back to an online check.
check "Ticket stapled to TimeBox.app" xcrun stapler validate "$APP"
check "Ticket stapled to the DMG"     xcrun stapler validate "$DMG"

# 6. The DMG is what people actually download, so it gets its own verdict.
check "Gatekeeper accepts the DMG" \
  spctl -a -vvv -t open --context context:primary-signature "$DMG"

# Summary
printf '%s\n' "${bold}────────────────────────────────────────${off}"
if [ ${#failures[@]} -eq 0 ]; then
  printf '%s\n\n' "${green}${bold}All checks passed.${off}"
  echo "Artifact to attach:"
  echo "  $DMG  ($(du -h "$DMG" | cut -f1))"
  echo
  echo "Next — RELEASE.md §6, as a draft:"
  echo "  gh release create v${VERSION} \"$DMG\" --title \"TimeBox ${VERSION}\" --draft"
  echo
  echo "${dim}Local checks cannot replace the real one: copy that DMG to a"
  echo "different Mac, open it, and confirm no Gatekeeper warning before you"
  echo "take the release out of draft.${off}"
  exit 0
else
  printf '%s\n' "${red}${bold}${#failures[@]} check(s) failed:${off}"
  for f in "${failures[@]}"; do echo "  ${red}✗${off} $f"; done
  echo
  echo "${bold}Do not attach this DMG to a release.${off}"
  echo
  # Two very different failures share this exit path, so name which one it is.
  if ! printf '%s\n' "${failures[@]}" | grep -qv "DMG"; then
    echo "Only the DMG failed — TimeBox.app is notarized and stapled, so the"
    echo "build environment was correct. Tauri notarizes the .app and then"
    echo "builds the DMG around it; it never submits the DMG itself. Notarize"
    echo "the container too, with the §3 credentials exported:"
    echo
    echo "  xcrun notarytool submit \"$DMG\" \\"
    echo "    --key \"\$APPLE_API_KEY_PATH\" --key-id \"\$APPLE_API_KEY\" \\"
    echo "    --issuer \"\$APPLE_API_ISSUER\" --wait"
    echo "  xcrun stapler staple \"$DMG\""
    echo
    echo "Then re-run this script. No rebuild is needed — stapling attaches the"
    echo "ticket to the existing file and does not touch the app inside it."
  else
    echo "The app itself failed a check. Most likely cause: the signing and"
    echo "notarization environment from RELEASE.md §3 was not exported in the"
    echo "shell that ran the build — Tauri then signs without notarizing, or"
    echo "skips signing entirely, and only warns."
  fi
  exit 1
fi
