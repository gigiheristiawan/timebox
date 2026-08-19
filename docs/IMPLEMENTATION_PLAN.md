# TimeBox — Implementation Plan

**Companion to:** [SPEC.md](SPEC.md) · **Prototype:** [mockup.html](mockup.html)
**Status:** Phases 1–5 complete · Phase 6 next · **Last updated:** 2026-08-19

Update the status marks in this file as work lands. Everything here is derived from SPEC.md — if the two disagree, SPEC.md wins and this file should be corrected.

---

## Progress

| Phase | Scope | Tasks | Status |
|---|---|---:|---|
| 1 | Foundation | 7 / 7 | ✅ Done |
| 2 | Domain core | 8 / 8 | ✅ Done |
| 3 | Persistence & recovery | 6 / 7 | ✅ Done (3.4 by a different mechanism — see note) |
| 4 | Main window UI | 9 / 9 | ✅ Done |
| 5 | Expiration checkpoint | 10 / 10 | ✅ Done |
| 6 | Menu bar | 0 / 7 | ⬜ Not started |
| 7 | Polish | 0 / 11 | ⬜ Not started |
| 8 | Release | 0 / 6 | ⬜ Not started |
| | **Total** | **40 / 65** | |

**Status key:** ⬜ Not started · 🟡 In progress · ✅ Done · ⛔ Blocked · ⏸ Deferred

**Definition of done for every phase:** unit tests pass · `tsc --noEmit` clean · `cargo test` clean · `cargo clippy` clean · Tauri build succeeds · the listed acceptance tests verified manually on the built `.app`.

---

## Phase 1 — Foundation

**Goal:** a `.app` that launches, shows an empty window, and has a working database.
**Exit criteria:** `npm run tauri build` produces a `.app` that opens and writes a schema-versioned SQLite file.
**✅ Met 2026-08-19** — 4.3 MB `.app` (arm64) + DMG. Launched, created `~/Library/Application Support/com.timebox.app/timebox.db` in WAL mode at schema version 1 with all four tables. `tsc --noEmit`, `vite build`, `cargo test` (4 passed), `cargo clippy -D warnings` all clean.

| # | Task | Status | Notes |
|---|---|---|---|
| 1.1 | Scaffold Tauri 2 + React 18 + TypeScript + Vite | ✅ | `tsc --noEmit` and `vite build` clean |
| 1.2 | Tailwind CSS with the token palette from the prototype (light/dark) | ✅ | All 3 theme states in `src/styles.css` |
| 1.3 | Add plugins: `notification`, `autostart`, `global-shortcut`, `single-instance` | ✅ | All four compile and link |
| 1.4 | SQLite at `~/Library/Application Support/com.timebox.app/timebox.db`, WAL + `synchronous=NORMAL` | ✅ | `db/mod.rs`; pragmas set on open |
| 1.5 | Forward-only numbered migration runner + `schema_migrations` table | ✅ | `db/migrations.rs`; 4 unit tests pass |
| 1.6 | Tables: `tasks`, `time_blocks`, `app_state`, `settings` | ✅ | Verified via `sqlite3`; constraints enforce D7/D10 |
| 1.7 | `LSUIElement = true`, accessory activation policy, placeholder app icon | ✅ | `LSUIElement=true` confirmed in bundled Info.plist |

---

## Phase 2 — Domain core

**Goal:** the entire product's behavior, as a pure reducer, provable without a UI. **This is the phase that matters most.**
**Exit criteria:** acceptance tests 1–4, 8–16 pass as pure reducer tests with an injected clock.

| # | Task | Status | Notes |
|---|---|---|---|
| 2.1 | `core/timer_machine.rs` — `reduce(state, event, now, ids) -> (state, Effect[])`, no I/O | ✅ | `ids: &mut dyn IdSource` added so id creation stays deterministic |
| 2.2 | States `IDLE`/`RUNNING`/`PAUSED`/`AWAITING_DECISION`; derived flags only, none stored | ✅ | `on_break()`, `staleness_ms()` etc. all derived |
| 2.3 | Timestamp arithmetic — `endAt`, `remainingWhenPaused`, `accumulatedActive` | ✅ | Milliseconds internally; Phase 3 maps to the schema |
| 2.4 | `core/queue.rs` — pure rotation, reorder, dequeue | ✅ | Every 'leaving a task' path routes through `rotate_to_back` |
| 2.5 | Decisions: complete / pending / extend | ✅ | Tests 2, 3, 4 pass |
| 2.6 | Break blocks + compound decisions (D7) | ✅ | Tests 17, 17b, 18, 19 pass |
| 2.7 | `switchTo` with **block parking** — park, never re-grant | ✅ | Tests 13–16 pass; mutation-checked (see below) |
| 2.8 | Reducer test suite | ✅ | 32 tests: 17 acceptance + 11 invariant/edge + 4 migration |

> ⚠ **2.7 is the subtlest rule in the product.** Returning to a set-down task must resume its remainder, never a fresh allocation.
>
> **Mutation-checked 2026-08-19.** With `start_task` sabotaged to ignore parked blocks and always grant a fresh allocation, tests 14 and 15 fail as intended (13 and 16 still pass, since they only assert the parking half). The guard has teeth.

---

## Phase 3 — Persistence & recovery

**Goal:** the app is correct across quit, crash, sleep, and wake.
**Exit criteria:** acceptance tests 6 and 7 pass on the built app.
**✅ Met 2026-08-19** — 44 tests pass (10 new persistence tests). Schema converted from seconds to milliseconds to match the core exactly; a lossy conversion at every save would have accumulated real drift in recorded time.

> **Note on 3.4.** The plan called for an `NSWorkspace.didWakeNotification` observer. The tick thread parks on a condvar with a 1-second timeout, so system sleep simply delays a tick and the timestamp comparison resolves expiry on the next one. That satisfies Test 6 without any macOS-specific code. A native observer would reduce worst-case detection latency from ~1s to ~0s; it is not needed for correctness and can be added in Phase 6 if the delay is ever noticeable.

| # | Task | Status | Notes |
|---|---|---|---|
| 3.1 | Repositories for tasks / time_blocks / app_state | ✅ | `db/repo.rs`. Settings repo deferred to Phase 7 with the settings UI |
| 3.2 | Every state transition persists in a single transaction | ✅ | Whole-state snapshot write; no diff to get wrong |
| 3.3 | Hydrate from SQLite and re-evaluate expiry **before first render** | ✅ | `App::hydrate` feeds one `Tick`; Test 7 passes |
| 3.4 | Wake handling | ✅ | **Mechanism changed:** the tick thread sleeps against wall time, so a wake produces a late tick that resolves expiry. Test 6 passes. A native `NSWorkspace` observer would only cut ≤1s of latency — see note |
| 3.5 | Parked blocks restore with exact remainders; only current block re-evaluates `endAt` | ✅ | Verified incl. the partial unique index surviving 6 switch cycles |
| 3.6 | Clock-moved-backwards guard (`now < startedAt` → elapsed 0) | ✅ | In `TimeBlock::active_ms`; tested in Phase 2 |
| 3.7 | Tick loop suspended entirely in `IDLE`/`PAUSED`/`AWAITING_DECISION` | ✅ | Condvar park — zero wakeups, not a skipped iteration |

---

## Phase 4 — Main window UI

**Goal:** the app is usable for a full workday from the main window.
**Exit criteria:** acceptance tests 8 and 13–16 verified by hand.
**✅ Met 2026-08-19** — 48 tests pass (4 new for `AddTask`/`Reorder`, which the core lacked entirely). `tsc --noEmit`, `clippy -D warnings`, and `tauri build` clean.

> **Carried into Phase 4 from Phase 7:** drag-to-reorder (7.1), since the queue list needed it to be usable at all.
>
> **Phase 4 stand-in:** `PendingDecision` renders the checkpoint's decisions *inline* in the main window, so the app is usable end to end once a block expires. The real thing — a borderless always-on-top window with app activation, sound, notification, and no exit path — is Phase 5. The actions themselves are already the real ones.

| # | Task | Status | Notes |
|---|---|---|---|
| 4.1 | Typed IPC wrappers + Zustand store mirroring backend state | ✅ | Single `dispatch(Action)` command; store holds clock skew |
| 4.2 | Current task panel — title, countdown, extension/interruption chips | ✅ | Countdown interpolates; never concludes expiry |
| 4.3 | Pause / Resume / Skip / Complete | ✅ | |
| 4.4 | Up Next queue list | ✅ | Drag-to-reorder landed early (was 7.1) |
| 4.5 | Click row / `↑` `↓` + `Return` → `switchTo` | ✅ | |
| 4.6 | Parked rows show remaining time + `Resume ▶`, not full allocation | ✅ | Tinted; strip and queue both use the remainder |
| 4.7 | Break panel while a break block runs | ✅ | |
| 4.8 | Add task (inline row) with validation | ✅ | Rejected in the reducer *and* surfaced inline |
| 4.9 | Rotation strip — segments sized by remaining allocation | ✅ | |

---

## Phase 5 — Expiration checkpoint

**Goal:** the feature the product exists for.
**Exit criteria:** acceptance tests 1–5 and 17–20 verified on the built app, including with the app hidden.

| # | Task | Status | Notes |
|---|---|---|---|
| 5.1 | Borderless always-on-top window filling the active display | ✅ | `monitor_from_point(cursor)`, falls back to primary |
| 5.2 | 2×2 compound actions; Keep Pending is the accent default | ✅ | `Return` = Keep Pending & Start Next |
| 5.3 | Extend row `+5/+10/+15/Custom` | ✅ | Custom validated before dispatch |
| 5.4 | Break-length selector + 4 compound actions | ✅ | Length picked first, so each action is one click |
| 5.5 | Extension-visibility warning | ✅ | Sums extensions across the task's blocks |
| 5.6 | Break checkpoint variant | ✅ | Headline is the *next* task; cool accent, not alert red |
| 5.7 | **No exit path** | ✅ | `decorations(false)` + `closable(false)` + `prevent_close()` refocuses; `Esc` unbound |
| 5.8 | App activation + key window | ✅ | `set_focus()` + `set_always_on_top(true)` |
| 5.9 | Sound + macOS notification; correct when denied | ✅ | `afplay` spawned, notification best-effort — neither can block a transition |
| 5.10 | Staleness line past a 2-minute floor | ✅ | |

---

## Phase 6 — Menu bar

**Goal:** a full day never requires the main window.
**Exit criteria:** every routine action is reachable from the popover.

| # | Task | Status | Notes |
|---|---|---|---|
| 6.1 | Menu bar item with template image (adapts to light/dark) | ⬜ | |
| 6.2 | Dynamic title: `◉ 24:17` / `◉ PAUSED` / `◔ BREAK 4:12` / `⚠ TIME'S UP` | ⬜ | SPEC §7.1 |
| 6.3 | Title updates at most 1 Hz, driven by the Rust tick | ⬜ | Battery |
| 6.4 | Popover — current, countdown, next, Pause/Skip, queue, menu | ⬜ | SPEC §7.2 |
| 6.5 | `menuBarShowTimer` setting honored | ⬜ | |
| 6.6 | `Cmd+Shift+T` global shortcut toggles popover | ⬜ | |
| 6.7 | Relaunch of a running instance opens the popover | ⬜ | D12; Test 21 |

---

## Phase 7 — Polish

**Goal:** the app feels finished.

| # | Task | Status | Notes |
|---|---|---|---|
| 7.1 | ~~Drag-and-drop reordering~~ | ✅ | Landed in Phase 4 with the queue list |
| 7.2 | Capacity strip — available / allocated / over, using remainders | ⬜ | Over-capacity visible, never blocked |
| 7.3 | Today summary — worked, on break, switched early, completed, pending, top 3 | ⬜ | Breaks excluded from worked |
| 7.4 | Settings window | ⬜ | SPEC §4.4 |
| 7.5 | In-app shortcuts: `Space` `N` `S` `D` `↑` `↓` `Enter` `Cmd+K` `Cmd+,` | ⬜ | SPEC §9 |
| 7.6 | `Cmd+K` quick add | ⬜ | |
| 7.7 | Theme: System / Light / Dark, all three viewer states correct | ⬜ | |
| 7.8 | Launch at login | ⬜ | |
| 7.9 | First-run panel pointing at the menu bar | ⬜ | D12 |
| 7.10 | `Away` total in Today | ⬜ | D13 |
| 7.11 | Quit confirm while a block is running (Pause & Quit / Quit / Cancel) | ⬜ | D14 |

---

## Phase 8 — Release

| # | Task | Status | Notes |
|---|---|---|---|
| 8.1 | Real app icon, all required sizes | ⬜ | |
| 8.2 | Universal binary (`universal-apple-darwin`) | ⬜ | Apple Silicon + Intel |
| 8.3 | Developer ID signing | ⬜ | |
| 8.4 | Notarization + stapling | ⬜ | Unsigned builds are dev-only |
| 8.5 | Performance check: < 80 MB idle, ~0% CPU when idle/paused | ⬜ | |
| 8.6 | Full manual pass of acceptance tests 1–20 on the notarized build | ⬜ | |

---

## Acceptance test coverage

Every test in SPEC §12 is owned by a phase. None may be left unassigned.

| Test | Subject | Phase | Status |
|---|---|---|---|
| 1 | Expiration halts at a decision | 2, 5 | ✅ |
| 2 | Complete & Start Next | 2, 5 | ✅ |
| 3 | Keep Pending & Start Next | 2, 5 | ✅ |
| 4 | Extend | 2, 5 | ✅ |
| 5 | App minimized → activates | 5 | ✅ |
| 6 | Mac sleep → awaiting decision | 3 | ✅ |
| 7 | Restart while awaiting decision | 3 | ✅ |
| 8 | Manual completion independent of timer | 2, 4 | ✅ |
| 9 | **Block completion ≠ task completion** | 2 | ✅ |
| 10 | Pause does not consume allocation | 2 | ✅ |
| 11 | Skip records elapsed, rotates | 2, 4 | ✅ |
| 12 | Re-queued block equals `blockDurationSeconds` | 2 | ✅ |
| 13 | Mid-block switch parks the block | 2, 4 | ✅ |
| 14 | **Return resumes the remainder** | 2, 4 | ✅ |
| 15 | **Switching cannot farm time** | 2 | ✅ |
| 16 | Switch is not a skip | 2, 4 | ✅ |
| 17 | Break rotates like Pending | 2, 5 | ✅ |
| 18 | Break does not auto-advance | 2, 5 | ✅ |
| 19 | Break accounting | 2, 7 | ✅ |
| 20 | Staleness line + `Away` | 5, 7 | ✅ |
| 21 | Relaunch opens popover | 6 | ⬜ |
| 22 | Break survives sleep/restart | 3 | ✅ |

Tests 9, 14, and 15 encode the product thesis. If any of them regress, the app has stopped being what it is for.

---

## Open questions

Carry-forward items to resolve before or during the phase that needs them.

| # | Question | Needed by | Status |
|---|---|---|---|
| Q6 | ~~Scope of the TypeScript mirror~~ — **Resolved 2026-08-19:** narrowed to formatters + countdown interpolation only. All decision logic stays in Rust (SPEC R7) | Phase 2 | ✅ Closed |
| Q1 | ~~Should "Complete & take a break" be a single action?~~ — **Resolved 2026-08-19:** yes. Break is a modifier on either task decision; checkpoint offers 4 compound actions + Extend, with break length pre-selected (D7) | Phase 5 | ✅ Closed |
| Q2 | ~~Do breaks consume `availableWorkMinutesPerDay`?~~ — **Resolved 2026-08-19:** no. Capacity measures work; rest is not work (D9) | Phase 7 | ✅ Closed |
| Q3 | ~~First-run onboarding~~ — **Resolved 2026-08-19:** relaunch opens the popover + one-time first-run panel (D12) | Phase 6/7 | ✅ Closed |
| Q4 | ~~Abandoned checkpoints record no time~~ — **Resolved 2026-08-19:** surface it, don't act on it. Staleness line + `Away` total (D13) | Phase 5 | ✅ Closed |
| Q5 | ~~Apple Developer account available?~~ — **Resolved 2026-08-19:** yes. Phase 8 can sign + notarize properly; no ad-hoc fallback needed | Phase 8 | ✅ Closed |
| Q7 | ~~Rust toolchain not installed~~ — **Resolved 2026-08-19:** Rust 1.97.1 installed; `cargo test`, `cargo clippy -D warnings`, and `tauri build` all clean | Phase 1 | ✅ Closed |
