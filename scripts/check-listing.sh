#!/usr/bin/env bash
# Character counts for appstore/listing.md against App Store Connect's limits.
# Overrunning is silent in the web form — it just truncates or refuses to save.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 - <<'PY'
import re, sys
src = open('appstore/listing.md').read()
LIMITS = {'Promotional Text': 170, 'Description': 4000, 'Keywords': 100, 'Copyright': 200}
ok = True
for name, limit in LIMITS.items():
    m = re.search(r'^## ' + re.escape(name) + r'.*?$\n(.*?)(?=^---|\Z)', src, re.S | re.M)
    if not m:
        print(f'{name}: SECTION NOT FOUND'); ok = False; continue
    body = m.group(1)
    body = re.sub(r'^\s*The app name is already indexed.*', '', body, flags=re.S | re.M)
    text = '\n'.join(l[4:] if l.startswith('    ') else l for l in body.strip().splitlines()).strip()
    n = len(text)
    flag = 'OK ' if n <= limit else 'OVER'
    if n > limit: ok = False
    print(f'{flag} {name}: {n}/{limit}')
sys.exit(0 if ok else 1)
PY
