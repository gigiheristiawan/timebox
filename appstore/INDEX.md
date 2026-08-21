# App Store submission — TimeBox

Everything needed to submit TimeBox to the Mac App Store, one file per section
of App Store Connect. Paste-ready; character limits are noted where they apply
and verified by `scripts/check-listing.sh`.

Build and packaging live in [docs/RELEASE.md §7](../docs/RELEASE.md), not here.

---

## Status

| Section | File | Required | Done |
| --- | --- | --- | --- |
| Version page — 0.1.0 | [listing.md](listing.md) | yes | |
| Screenshots | [screenshots/](screenshots/) | yes | ✅ generated |
| General → App Information | [app-information.md](app-information.md) | yes | |
| General → App Review | [review-notes.md](review-notes.md) | yes | |
| Trust & Safety → App Privacy | [app-privacy.md](app-privacy.md) | yes | |
| Trust & Safety → App Accessibility | [accessibility.md](accessibility.md) | no | skip for 0.1.0 |
| Monetization → Pricing and Availability | [pricing-and-availability.md](pricing-and-availability.md) | yes | |
| Monetization → In-App Purchases | — | no | none |
| Monetization → Subscriptions | — | no | none |
| Growth → Promo Codes | — | no | free app |
| Growth → Game Center | — | no | not a game |
| Featuring → Nominations | — | no | optional |

---

## The files

### [listing.md](listing.md)
The 0.1.0 version page: promotional text, description, keywords, support and
marketing URLs, copyright. **Also carries the version-number trap** — App Store
Connect defaults the field to `1.0` while the uploaded build is `0.1.0`, and the
two must match.

### [screenshots/](screenshots/)
Four 2880×1800 PNGs with no alpha channel, both of which App Store Connect
enforces. Regenerate with `./scripts/make-screenshots.sh`, which renders them
from `docs/mockup.html` so they track the design rather than drifting from it.

### [app-information.md](app-information.md)
Name, subtitle, categories, content rights, age rating and license agreement.
Flags the **Unrestricted Web Access** question, where answering Yes because the
app uses a web view would force a 17+ rating.

### [review-notes.md](review-notes.md)
Review contact, notes, and the App Sandbox entitlement justification. Written to
pre-empt the two likely rejections: a menu bar app that "does nothing" on launch,
and a checkpoint window a reviewer cannot close.

### [app-privacy.md](app-privacy.md)
"Data Not Collected", with the source-level evidence for why that answer holds
and why the `network.client` entitlement does not contradict it.

### [pricing-and-availability.md](pricing-and-availability.md)
Free, all territories. Notes that the **Free Apps agreement** must be active or
the section silently refuses to save.

### [accessibility.md](accessibility.md)
Optional, and recommended **blank** for 0.1.0 — with what exists today and what
to test before claiming anything.

---

## Order to work through

1. **Pricing and Availability** first. It is the section most likely to be blocked
   by something outside the form (the Free Apps agreement).
2. **App Information** — the app-level settings every version inherits.
3. **App Privacy** — short, but submission is blocked without it.
4. **Version page** — description, keywords, screenshots, and attach build 2.
5. **App Review** — contact details, notes, sandbox justification.
6. Submit.
