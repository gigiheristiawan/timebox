# App Privacy  (App Store → Trust & Safety → App Privacy)

Mandatory. Cannot submit without it.

---

## Privacy Policy URL

    https://gigiheristiawan.github.io/timebox/privacy.html

---

## Data Collection

> Do you or your third-party partners collect data from this app?

    No, we do not collect data from this app

Choosing this collapses the entire questionnaire — there are no data types to
declare, and the product page will show **"Data Not Collected"**.

### Why this answer is defensible

This was checked against the source, not assumed. TimeBox has:

- no HTTP client crate and no networking plugin in the Rust binary
- no `fetch`, `XMLHttpRequest`, `WebSocket` or beacon call anywhere in the frontend
- no analytics, crash reporting, attribution or advertising SDK
- no auto-updater
- a webview capability list that grants no network permission

Everything the app stores is a local SQLite database inside its sandbox
container. Nothing is transmitted, so nothing is collected.

### The one thing that looks like a contradiction

The app declares `com.apple.security.network.client`. That is a *capability*
grant required by WKWebView to start its renderer inside the sandbox — without
it the window renders blank. It is not data collection, and it does not change
this answer. The public privacy policy discloses it explicitly so that anyone
inspecting the entitlements finds an explanation rather than a surprise.
