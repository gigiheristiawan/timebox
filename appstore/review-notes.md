# App Review Information

---

## Checkboxes

- **Game Center** — leave unchecked.
- **Sign-in required** — leave unchecked. TimeBox has no accounts.
- **Attachment** — not needed.

## Contact Information

Your own name, phone and email. This is private to Apple and never appears on
the listing, so the email here is fine even though the public privacy page
deliberately has none.

---

## Notes  (max 4000)

TimeBox is a menu bar utility. No account or sign-in is required and there is nothing to purchase.

WHERE THE APP IS

TimeBox has no Dock icon and opens no window at launch (LSUIElement is set). After launching, look for the TimeBox mark in the menu bar at the top right of the screen. Click it to open the popover. Cmd+Shift+T opens it from anywhere.

HOW TO EXERCISE IT IN ABOUT TWO MINUTES

1. Click the menu bar icon, then "Open App".
2. Type a task name in the field at the bottom, pick a duration, click Add.
3. Click the task to start its time block. The menu bar shows a live countdown.
4. Click "Skip block" to rotate to the next task immediately, or let a block run out to reach the checkpoint described below.

ABOUT THE CHECKPOINT — PLEASE READ BEFORE TESTING

When a time block expires, TimeBox shows a checkpoint window that intentionally has no dismiss button, no close button, no snooze and no timeout. The Escape key does nothing.

This is the core feature of the app, not a defect. The product exists to force one explicit decision about a task when its allotted time runs out, and silently advancing would defeat its entire purpose. The window always offers five ways forward — complete the task, complete it and take a break, keep it pending, keep it pending and take a break, or extend the block — and it continues as soon as one is chosen.

The app remains fully quittable at every moment, including while a checkpoint is open: Cmd+Q, or Quit from the menu bar popover.

NETWORK ACCESS

TimeBox makes no network connections and contains no networking code — no HTTP client, no analytics, no crash reporting and no auto-updater. The com.apple.security.network.client entitlement is present only because WKWebView, which draws the app's interface, will not start its renderer process inside the App Sandbox without it; the window renders blank. No data leaves the device.

PRIVACY

No accounts, no analytics, no tracking. All data is stored in a local SQLite database inside the app's sandbox container.

Source code: https://github.com/gigiheristiawan/timebox

---

## App Sandbox Information

Declare the one entitlement that is not self-explanatory:

**Entitlement:** `com.apple.security.network.client`

**Reason:**

Required by WKWebView, which renders the app's user interface. Inside the App Sandbox, WebKit does not launch its WebContent and Networking helper processes without this entitlement, and the application window renders blank as a result. TimeBox itself opens no network connections and contains no networking code; nothing is transmitted off the device.

`com.apple.security.app-sandbox` needs no justification — it is the sandbox itself.
