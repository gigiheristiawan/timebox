# TimeBox for Windows — Port Specification

Scope: ship the **same product**, full feature parity with the macOS build, from the
same repository and the same Rust core. This document specifies only what differs.
`docs/SPEC.md` stays authoritative for product behaviour — decisions `D1`–`D14`,
the state machine, and acceptance tests 1–22 are unchanged and must pass
identically on Windows. Nothing here may add, remove, or soften a product rule.

Target: **Windows 11 22H2+ (primary), Windows 10 21H2+ (supported)**, x64 and
arm64. WebView2 Evergreen runtime required.

---

## Changelog

| Date (WIB)       | Change            |
| ---------------- | ----------------- |
| 2026-08-21 | Initial version. |

---

## 1. What ports unchanged

These carry over with **zero Windows-specific code**, and that is the point of the
existing architecture:

- `core/` — reducer, queue ops, model, `menubar.rs`, `summary.rs`. Pure, no I/O,
  no clock, no Tauri. Every acceptance test in `core::tests` runs on Windows as-is.
- `db/` — rusqlite `bundled` compiles on MSVC; migrations, `repo::save`,
  `settings.rs` unchanged. Only the *path* changes (§7).
- `state.rs` — `hydrate` + one `Tick`, the condvar tick thread, `day_start_ms`
  (chrono `clock` feature reads the Windows timezone correctly).
- `commands.rs` — the whole IPC surface is portable.
- `src/` (React/TS) — one platform branch for modifier keys and copy (§6),
  otherwise identical.

**Invariant:** no product decision may move into a platform module. If a Windows
surface seems to need a rule, the rule is missing from `core/`, not from Windows.

---

## 2. Module layout

`platform/` gains an OS split behind one facade, so `lib.rs` and `commands.rs`
never `cfg` on the OS:

```
platform/
  mod.rs            re-exports the facade; #[cfg(target_os)] picks the impl
  checkpoint.rs     shared: apply(fx, settings) — effect dispatch is portable
  macos/            popover, tray, quit_confirm, window_corners, login_item
  windows/          popover, tray, quit_confirm, window_corners, login_item,
                    sound.rs, hud.rs
```

Facade (every item must exist on both, same signature):

| Function | macOS | Windows |
|---|---|---|
| `popover::{toggle,show,hide,remember_anchor}` | under the menu bar icon | above the tray icon, taskbar-aware (§3.2) |
| `tray::{init,refresh,refresh_forced,set_show_timer}` | title text | rendered icon + tooltip + optional HUD (§3.1) |
| `quit_confirm::{show,hide}` | same window | same window |
| `window_corners::round` | `setOpaque:NO` + layer radius | DWM corner preference / transparent window (§3.3) |
| `login_item::{set,is_enabled}` | `SMAppService` | Run key or `StartupTask` (§5) |
| `sound::play_expiry(kind)` | `afplay` Glass/Ping | `PlaySound` alias (§4.3) |

`platform/checkpoint.rs` stays shared — it only interprets `Effect`s and calls the
facade. A checkpoint reached from the tick loop or from `dispatch` must behave
identically on Windows too.

---

## 3. The three real problems

### 3.1 There is no menu bar title — the countdown has no home

The single largest gap. `TrayIcon::set_title` is **macOS-only** in Tauri 2; the
Windows shell notification area has no text label. SPEC §7.1's table
(`◉ 24:17`, `⚠ TIME'S UP`) has nothing to render into.

`core::menubar::title(state, now, show_timer)` stays the source of truth for the
string. Windows renders it in three tiers:

1. **Icon state (always).** `tray::refresh` composites a 32×32 RGBA icon and calls
   `set_icon`, at most 1 Hz and only when the rendered state changes:
   - IDLE — the plain mark.
   - RUNNING (work) — mark plus a progress ring, filled by elapsed/allocated.
   - RUNNING (break) — same ring in the rest accent (cool blue).
   - PAUSED — mark plus a pause glyph.
   - AWAITING_DECISION — alert-red mark; **plus a flash cadence** (alternate two
     variants at 1 Hz for the first 10s) since there is no `⚠ TIME'S UP` text.

   Render with `tiny-skia` into an `Image::new_owned`. **Do not draw digits into
   32px** — mm:ss is illegible at tray size, and a wrong-looking countdown is
   worse than none. The icon is **not** a template image: Windows does not recolor
   tray icons, so ship a light-theme and a dark-theme variant and pick from
   `ShouldAppsUseLightTheme` (registry, `AppsUseLightTheme`), re-picking on
   `WM_SETTINGCHANGE`.

2. **Tooltip (always).** `set_tooltip(core::menubar::title(...))`, refreshed on the
   same 1 Hz gate. Exact text parity with the macOS menu bar, minus the glyphs.

3. **HUD window (opt-in, new setting).** Because tiers 1–2 lose the at-a-glance
   countdown macOS users get for free, Windows adds one setting,
   `windowsShowHud` (default **on** when `menuBarShowTimer` is true, ignored
   otherwise): a 148×44 always-on-top, click-through-except-drag, no-taskbar
   window showing `◉ 24:17`, remembered position, hidden in IDLE. It is
   presentation only — it dispatches nothing and holds no state; it reads the same
   snapshot the popover does.

   **Migration 003** adds `settings.windows_show_hud` and
   `settings.hud_x` / `hud_y` (nullable). Forward-only, per the existing rule. The
   column exists on macOS too and is simply unused there — one schema, one
   `Settings` struct, no per-OS branching in `db/`.

`menuBarShowTimer` keeps its name in the schema and its meaning ("show the
countdown outside the app"); the Windows settings UI labels it "Show timer in the
tray".

### 3.2 The popover is anchored to a taskbar that moves

macOS pins the menu bar to the top. Windows' taskbar can be on any edge, can be
auto-hidden, and Windows 11 hides most tray icons in an overflow flyout.

- Anchor from the `TrayIconEvent::Click` `rect` exactly as today (it is populated
  on Windows and is already physical pixels). Keep the remembered-anchor and
  300ms reopen-guard logic verbatim.
- Placement flips by taskbar edge: resolve the work area
  (`SystemParametersInfo(SPI_GETWORKAREA)`) for the monitor holding the anchor and
  place the popover **inside the work area**, on the opposite side of the icon from
  the screen edge — above the icon for a bottom taskbar, below for a top one, left
  or right for a side taskbar. Clamp on both axes (today only x is clamped).
- No anchor yet (global shortcut, relaunch): the work area's tray-side corner, not
  the top-right.
- **Overflow.** Icons live in the hidden flyout by default on Windows 11 and cannot
  be pinned programmatically. The first-run panel (D12) must say so and show how to
  pin: drag the icon out of the `^` overflow. This is the direct analogue of the
  notch problem, and `Ctrl+Shift+T` is the same recovery path.
- Focus-loss dismissal (`WindowEvent::Focused(false)`) behaves the same.

### 3.3 Rounded corners and transparency

`macos-private-api` is meaningless here; Tauri's `transparent(true)` is public API
on Windows and works with WebView2.

- **Windows 11:** build opaque, then `DwmSetWindowAttribute` with
  `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND` on the popover, quit-confirm and
  HUD. Cheapest, correct shadow, no transparency cost.
- **Windows 10:** no DWM rounding. Use `transparent(true)` + the CSS radius the
  card already has. Accept the missing drop shadow rather than faking one.
- The checkpoint window is opaque and full-screen; it gets neither.

---

## 4. Windows surfaces, one by one

### 4.1 No Dock icon → no taskbar button

`set_activation_policy(Accessory)` has no Windows equivalent. Parity is:
`skip_taskbar(true)` on **every** window including `main`, and no window shown at
launch. The app is reachable only from the tray icon and `Ctrl+Shift+T` — which is
why §3.2's overflow guidance and the first-run panel are load-bearing, not polish.

Closing `main` hides it (unchanged). `Alt+F4` on `main` routes through
`CloseRequested` and is therefore also a hide, matching `Cmd+W`.

### 4.2 The checkpoint window

Same rules, same no-exit guarantee. Windows specifics:

- Borderless, `skip_taskbar(true)`, sized to the **work area** of the monitor under
  the cursor — *not* the full monitor bounds; covering an auto-hide taskbar
  prevents it from being revealed and reads as a hang.
- `always_on_top(true)` plus `SetForegroundWindow`. Windows' foreground lock can
  refuse activation from a background process; when it does, fall back to
  `FlashWindowEx(FLASHW_ALL | FLASHW_TIMERNOFG)` so the taskbar/tray flashes. Never
  loop on `SetForegroundWindow` — it is the "steal focus" pattern the OS
  deliberately blocks.
- **Exclusive-fullscreen apps (games, some players) can render over a topmost
  window.** This is unavoidable without hooks the app will not use. The checkpoint
  still exists, still blocks, and is there when the user leaves fullscreen; the
  sound and toast are what reach them meanwhile. Document it; do not work around it.
- `CloseRequested` → `prevent_close()` + refocus, covering `Alt+F4`, the taskbar
  context menu and `WM_CLOSE`. `Esc` inert. No system menu (`decorations(false)`
  already removes it).

### 4.3 Sound

`afplay` does not exist. `platform/windows/sound.rs` plays the system aliases via
`PlaySound(SND_ALIAS | SND_ASYNC)`:

| macOS | Windows |
|---|---|
| `Glass` (work expiry) | `SystemExclamation` |
| `Ping` (break over) | `SystemNotification`, falling back to `SystemAsterisk` |

Async and best-effort, exactly as today: a muted or missing sound must never delay
or fail a transition.

### 4.4 Notifications

`tauri-plugin-notification` works on Windows via WinRT toasts, but a toast requires
a Start Menu shortcut carrying an AppUserModelID. The NSIS/MSI bundles Tauri
generates create it; **`tauri dev` does not**, so toasts are silently absent in
development. That is a dev-loop artifact, not a bug — verify toasts on an installed
build only. Windows has no permission prompt, so the "denied permission" path from
SPEC §8 simply never triggers; the fallback code stays.

### 4.5 Quit confirmation and single instance

`RunEvent::ExitRequested` fires on Windows, so D14's confirm hooks unchanged.
`tauri-plugin-single-instance` is fully supported (named mutex, no `/tmp` socket
concern) — keep it, and keep `RunEvent::Reopen` guarded to macOS.

---

## 5. Launch at login

Two mechanisms, chosen by distribution channel:

- **Installer builds (NSIS/MSI):** `tauri-plugin-autostart`, which writes
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. No sandbox, so the reason
  it was removed on macOS does not apply. Pass `--minimized`-equivalent by
  launching with no window shown (already the default).
- **Microsoft Store (MSIX, optional):** the Run key is stripped in the AppContainer.
  Declare a `windows.startupTask` extension in the package manifest and toggle it
  with `StartupTask.RequestEnableAsync`. `login_item::set` returns the same
  `Result<(), String>`, and the caller keeps treating failure as non-fatal with the
  stored preference as the truth.

`is_enabled()` should report the *system's* view in both cases, so the settings UI
can detect a user who disabled the entry in Task Manager → Startup.

---

## 6. Keyboard

Mechanical `Cmd`→`Ctrl` substitution, plus one collision check.

| macOS | Windows |
|---|---|
| `Cmd+Shift+T` (global toggle) | `Ctrl+Shift+T` — **verify at registration**; browsers own it in-focus only, so app-wide registration wins, but if the plugin reports it taken, fall back to `Ctrl+Alt+T` and say so in Settings rather than failing silently |
| `Cmd+K` quick add | `Ctrl+K` |
| `Cmd+,` settings | `Ctrl+,` |
| `Cmd+W` (hides) | `Alt+F4` (hides) |

`src/App.tsx` currently tests `e.metaKey` directly. Introduce
`src/core/platform.ts` exporting `isMac` (from `navigator.userAgentData?.platform`,
resolved once) and `accel(e) = isMac ? e.metaKey : e.ctrlKey`; every handler uses
`accel`. Unmodified single keys (`Space`, `N`, `S`, `D`, `1`–`5`, `Return`) are
unchanged. Copy: `⌘⇧T` in `FirstRun.tsx` and the shortcut hints become
platform-conditional strings, not hardcoded glyphs.

---

## 7. Paths, storage, packaging

- Data dir: `app_data_dir()` → `%APPDATA%\xyz.gigiheristiawan.timebox\`. No code
  change; `Db::open` already takes the resolved dir.
- Identifier and product name unchanged, so a user with both machines has the same
  schema and the same file name.
- **WebView2 Evergreen** is a runtime dependency. Ship the bundled bootstrapper
  (`"webviewInstallMode": { "type": "downloadBootstrapper" }`) rather than the
  offline installer, unless an offline-install requirement appears.
- Targets: `nsis` (primary, per-user install → no elevation, which the Run key and
  a tray utility both want) and `msi` (for managed deployment). Build arm64 and x64
  separately; there is no universal binary.
- **Code signing:** Authenticode. Without it SmartScreen blocks first run behind
  "More info → Run anyway", which for a menu-bar-style utility reads as broken.
  Use Azure Trusted Signing or an EV certificate; reputation accrues per publisher,
  so sign every release with the same identity. Document the exact commands in a
  Windows section of `docs/RELEASE.md`, mirroring the notarization section.
- Tray icon assets: Windows needs `.ico` with 16/20/24/32/48/256 px frames for the
  app icon (`tauri icon` generates `icon.ico`), plus the two tray PNG variants from
  §3.1. The macOS template PNG cannot be reused — pure black would be invisible on
  a dark taskbar.

---

## 8. Build, test, CI

Rust cannot cross-compile the MSVC target from macOS in practice (WebView2 + link).
Add a GitHub Actions matrix:

```
macos-latest    → cargo test, clippy, tauri build (dmg)
windows-latest  → cargo test, clippy -D warnings, tauri build (nsis)
```

`core::tests` and `state/tests.rs` must be green on both — they are the proof that
no product rule drifted. `login_item`'s existing "unbundled process is refused"
test needs a Windows sibling asserting the Run key round-trips under `HKCU`.

Rule 16 extends: on Windows too, `cargo test` + `cargo clippy` + `npm run typecheck`
is the loop; a build proves nothing about a window you cannot see.

---

## 9. Windows acceptance tests

Numbered `W1`–`W10`, run manually on an **installed** build, in addition to the
unchanged 1–22.

1. **W1 Tray state** — start a 1m block: icon shows a filling ring; tooltip matches
   `core::menubar::title`; at expiry the icon turns alert-red and flashes.
2. **W2 Overflow discovery** — with the icon in the `^` flyout, `Ctrl+Shift+T` opens
   the popover under/near the flyout and it is fully on screen.
3. **W3 Taskbar edges** — move the taskbar to top, left and right, and auto-hide it:
   the popover stays inside the work area on all four, on both monitors.
4. **W4 Checkpoint focus** — expire a block while typing in another app: the
   checkpoint fills the work area of the monitor with the cursor and takes focus, or
   flashes the taskbar if the OS refuses.
5. **W5 No exit** — at a checkpoint, `Esc`, `Alt+F4`, `Ctrl+W`, taskbar context menu
   → Close, and clicking outside all leave it open.
6. **W6 Sleep and hibernate** — sleep past `end_at`, and separately hibernate: on
   resume the checkpoint is present with a correct staleness line (tests 6 and 20).
7. **W7 Fast startup** — Windows fast startup / an OS update reboot restores the
   state exactly, including a parked block's remainder.
8. **W8 Launch at login** — toggle on, sign out and back in: TimeBox starts with no
   window and a tray icon; toggle off removes the Run entry.
9. **W9 Second launch** — running the exe again opens the popover, does not start a
   second instance against the same database (test 21).
10. **W10 DPI** — with a 150% primary and 100% secondary monitor, the popover, HUD
    and checkpoint are correctly sized and positioned on both, including after
    dragging a window across.

---

## 10. Explicitly out of scope

- Any attempt to lock the desktop, block other applications, install hooks, or
  cover exclusive-fullscreen apps. SPEC §7.4's limit is a product rule, not a macOS
  limitation.
- A Linux target. The facade in §2 makes one cheap later; nothing here assumes it.
- Windows-only features. Parity means parity — the HUD (§3.1) is the one addition,
  and it exists to *restore* a macOS affordance the platform lacks, not to extend
  the product.
