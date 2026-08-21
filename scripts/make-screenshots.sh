#!/usr/bin/env bash
# Render App Store screenshots from docs/mockup.html.
#
# The mockup is the design reference and already contains every surface in a
# real, working state, so screenshots come from it rather than from staged
# captures of the running app: they are reproducible, deterministic, and they
# update when the design does.
#
# Output is 2880x1800 — one of the four sizes App Store Connect accepts for
# macOS — with no alpha channel, which it rejects.
set -euo pipefail

cd "$(dirname "$0")/.."

CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
OUT=appstore/screenshots
HARNESS=appstore/harness.html.part
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

[ -x "$CHROME" ] || { echo "error: no Chrome at $CHROME (override with \$CHROME)" >&2; exit 1; }
[ -f "$HARNESS" ] || { echo "error: missing $HARNESS" >&2; exit 1; }

# mockup.html has no </body>; the harness is appended so it runs *after* the
# prototype's own startup, then hides the page chrome, freezes the virtual
# clock, drives the state for one shot, and lays out the caption band.
cat docs/mockup.html "$HARNESS" > "$WORK/shot.html"

mkdir -p "$OUT"
for shot in running checkpoint popover settings; do
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --no-sandbox \
    --virtual-time-budget=2000 --force-device-scale-factor=2 --window-size=1440,900 \
    --screenshot="$OUT/$shot.png" \
    "file://$WORK/shot.html?shot=$shot&theme=light" >/dev/null 2>&1
  w=$(sips -g pixelWidth  "$OUT/$shot.png" | awk -F': ' '/pixelWidth/{print $2}')
  h=$(sips -g pixelHeight "$OUT/$shot.png" | awk -F': ' '/pixelHeight/{print $2}')
  a=$(sips -g hasAlpha    "$OUT/$shot.png" | awk -F': ' '/hasAlpha/{print $2}')
  [ "$w" = "2880" ] && [ "$h" = "1800" ] || { echo "error: $shot is ${w}x${h}, expected 2880x1800" >&2; exit 1; }
  [ "$a" = "no" ] || { echo "error: $shot has an alpha channel; App Store Connect rejects those" >&2; exit 1; }
  echo "  $OUT/$shot.png  ${w}x${h}"
done

echo
echo "Upload these in App Store Connect under the macOS screenshots section."
