# Timer overlay

The timer is readable in two places, and both of them require leaving the work:
the menu bar title, which the notch can hide entirely, and the popover, which is
a click away and closes the moment focus moves. Neither is visible from inside
the editor, the browser or the call you are actually in.

This spec adds a **floating overlay**: a small always-on-top card on the main
display showing the current block — the countdown and the task, a break, or the
fact that nothing is running at all. It is read-only, and it is off until it is
asked for.

Extends `docs/SPEC.md`. Decisions continue the numbering from D46
(`WEEKLY_REPORT.md`); acceptance tests continue from 101.

Status: **implemented** (issue
[#23](https://github.com/gigiheristiawan/timebox/issues/23)).

---

## Changelog

| Date (WIB)       | Change                                                            |
| ---------------- | ----------------------------------------------------------------- |
| 2026-09-10 11:35 | **A running break read as idle (issue #27).** A break block carries no task, and `!task` was the card's whole test for *nothing running*, so it painted today's idle total over the break countdown. §5. |
| 2026-09-10 11:27 | **The card never updated (issue #27).** `overlay` was missing from `capabilities/default.json`, so the window was refused `event.listen` and never saw a `timebox://changed`. §3.1. |
| 2026-09-10 09:55 | Initial version. D47–D52; migration 007; acceptance tests 102–106. |

---

## 1. The premise

The product's claim is that the timer controls how long you work on a task. That
claim only holds while the remaining time is *visible*: a countdown you have to
go and look for is a countdown you check when you already suspect it has run
out.

So the overlay is the timer where the work is. What it must not become is a
fourth surface with opinions of its own — every other window in the app can act,
and this one deliberately cannot.

---

## 2. Decisions

| #   | Decision | Reasoning |
| --- | -------- | --------- |
| D47 | **The overlay stays up when nothing is running**, showing today's idle total in place of a countdown. | The state the overlay is most useful in is the one the user has drifted out of: off the queue, nothing started, no clock anywhere claiming otherwise. A card that vanished at exactly that moment would take the reminder with it, and would read as a bug — the overlay disappears and nothing says why. Idle is already computed by `core::summary` for the Today strip (IDLE_TIME §3), so this is a second reader of one number, not a second definition of it. |
| D48 | **It is a display, not a control.** The window ignores the cursor entirely (`set_ignore_cursor_events`), so it can never take focus, never swallow a click meant for the window underneath, and offers no button, no drag and no close box. | An always-on-top window that can be clicked is a window the user has to manage — it lands on top of the thing being read, and the instinct is to drag it aside or close it. Neither is possible here, so neither has to be handled: the overlay is furniture. It also settles the position question — a card that cannot be dragged must be placed from the settings, which is why D51 is five named anchors rather than free coordinates. |
| D49 | **It hides while either checkpoint is open.** | The checkpoint fills the main display and has no exit (SPEC §7.4); the overlay sits on the same display. A card repeating "time's up" on top of the window already saying so is noise at the one moment the screen must carry exactly one thing. Both waiting states count — the Pomodoro prompt puts the same window up — which is the same trap `at_any_checkpoint` was introduced for (POMODORO_MODE D30). |
| D50 | **Opacity is the window's alpha, not the card's background colour.** | Fading only the fill would keep the text crisp, but it needs the webview itself to paint nothing behind the card, and the only reliable route to that on macOS is the private API the App Store rules out (`platform/window_corners.rs`). `NSWindow.setAlphaValue:` is public and certain. The text fades with the card, which is what a translucent overlay looks like anyway, and the slider says plainly that 0% is invisible. |
| D51 | **Five named anchors on the main display, and a fixed size.** The card is 200pt wide, and 96pt or 68pt tall depending only on whether the task title is shown. | Free coordinates are unreachable without dragging (D48). The main display is the same choice the checkpoint makes and for the same reason (issue #20): the cursor's screen is wherever the pointer was abandoned. The size is fixed rather than measured from the content because the window is anchored to a *corner* — a card that grew with a long title would have to be re-positioned as it resized, and a webview resizing itself while Rust re-anchors it is a visible wobble. Long titles truncate. |
| D52 | **The whole feature is four settings columns. No event, no state, no reducer change.** | Nothing about the overlay decides anything: it reads the snapshot every other window reads. Adding an `Action` to open or close it would put a second source of truth beside the stored setting and invite the two to disagree — instead `platform::overlay::reconcile` is called from every path that can change the settings or the state, and the window is purely a consequence of what is stored. |

### 2.1 What is deliberately absent

- **No per-display or multi-screen placement.** One card, on the main display.
- **No drag-to-position**, which D48 rules out by construction.
- **No click-through action** — not even "click to open the popover"; the window
  receives no clicks at all.
- **No separate overlay theme.** It paints the app's tokens, so it follows the
  Theme setting like every other surface.

---

## 3. Shape

```
settings.overlay_show             bool    off on upgrade (D48)
settings.overlay_show_task_title  bool    the half that leaks on a shared screen
settings.overlay_position         enum    TOP_LEFT | TOP_RIGHT | BOTTOM_LEFT
                                          | BOTTOM_RIGHT | CENTER
settings.overlay_opacity_pct      0..=100 applied as the window's alpha (D50)
```

Migration **007** adds the four columns, each with a default, so an existing
install upgrades with the overlay off.

`platform/overlay.rs` owns the window: build once, then show, hide, place and
fade. `reconcile(app, settings, state)` is called from four places — startup,
every `dispatch`, `update_settings`, and the tick loop — and is the only thing
that touches it. It compares the settings-derived `Look` against what was last
applied, so the once-a-second call from the ticker does no work while nothing
has changed.

`components/Overlay.tsx` paints the card and nothing else. The window's label
routes it in `src/main.tsx` like every other surface, and the countdown is the
shared `Countdown` component, so the overlay cannot disagree with the popover
about the time remaining.

### 3.1 The capability list

The overlay is a window, and **every window label must be named in
`src-tauri/capabilities/default.json`**. A window left out still runs the app's
own `#[tauri::command]`s — those need no permission — so it mounts, fetches its
snapshot and paints correctly, and then never changes again: `listen` is a
*core* command, it is refused, and no `timebox://changed` is ever delivered.
That was issue #27. The card looked alive because `Countdown` interpolates
locally, so it counted its first block down to 00:00 and stayed there, through
breaks, switches and checkpoints alike.

Nothing in the failure is visible from Rust: the emit succeeds, the window is
there, and the only signal is a rejected promise in a webview with no console
open. `useTimebox.init` now catches it, records the error and still installs the
10s poll — a refused listener costs the second-by-second nudge, not every
update. `settings` was missing from the same list and had the same defect; it
went unnoticed because that window is opened, changed and closed inside a few
seconds.

### 3.2 Units, again

The card is placed in **logical** points against the main monitor's frame,
converted with *that monitor's* scale factor — `checkpoint::main_monitor_frame`,
shared rather than re-derived. Handing Tauri's setters physical pixels sizes the
window by the window's own scale factor, which is the bug behind issue #20.

---

## 4. Acceptance tests

| #   | Test | Asserts |
| --- | ---- | ------- |
| 102 | `t102_the_overlay_is_off_until_it_is_asked_for` | Migration 007 upgrades an existing install with the overlay off, the title on, bottom-right, 85%. |
| 103 | `t103_an_out_of_range_opacity_is_clamped_rather_than_passed_to_appkit` | A stored 250% is clamped to 100 on write and on read — AppKit would take it and mean nothing by it. |
| 104 | `t104_the_position_encodings_are_the_database_one_and_the_wire_one` | `BOTTOM_RIGHT` in the column, `BottomRight` on the wire, both directions. A mismatch parks the card in the default corner and fails nothing. |
| 105 | `t105_every_position_is_accepted_by_the_column` | All five positions survive the round trip past 007's `CHECK` — the constraint fails at runtime, not at compile time (migration 006's lesson). |
| 106 | `t106_the_overlay_yields_to_either_checkpoint_and_stays_up_when_idle` | Hidden in `AwaitingDecision` and `AwaitingPomodoro` (D49); shown in `Idle`, `Running` and `Paused` (D47). |

---

## 5. Known limits

- **The overlay is a window, so a genuinely full-screen app can cover it.** It
  joins all Spaces and re-asserts always-on-top whenever it is shown, which
  covers ordinary Space switching; a native full-screen Space is macOS's call.
- **A break is not idle, though it has no task.** The card's "nothing running"
  test is `Idle`, or no task *and* no break — break blocks deliberately carry
  no task (SPEC), and testing `!task` alone showed today's idle total in place
  of the break countdown (issue #27).
- **Idle is shown as today's total, refreshed on the store's 10s stopped-state
  poll**, not interpolated. The number moves in minutes, so a second-accurate
  idle clock would be precision the figure does not have.
- **At 0% the card is invisible but still there** — deliberate, and the settings
  copy says so, but a user who slides to 0 and forgets has an overlay that looks
  broken. The toggle above it is the honest way off.
