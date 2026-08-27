# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Changelog

| Date (WIB)       | Change                                                                                               |
| ---------------- | ---------------------------------------------------------------------------------------------------- |
| 2026-08-27 09:40 | The popover's current-task row now offers **Complete** next to Pause and Skip (`components/Popover.tsx`), sending the same `completeCurrentTask` action as the main window (issue #7). No backend change — `Event::CompleteCurrentTask` already handled it. `docs/SPEC.md` §7.2 updated. |
| 2026-08-26 12:40 | Rules 18 and 19 moved out of this file into `skills/create_git_commit.md`, where the rest of the git rules already live. Rule 15 points at it. |
| 2026-08-26 12:20 | **Rule 19 added** — the `claude.ai/code/session_…` link never leaves the terminal. It went into both commits on PR #5 and the PR body, where it reads as Gigih's link on Gigih's repo and is permanent. `Co-Authored-By` is the whole of the attribution. |
| 2026-08-26 12:05 | **Rule 18 added** — ask every time before acting as Gigih. PR #5 was pushed and opened with his `gh` token without asking first; `skills/create_git_commit.md` said not to push, and "create the PR" was read as covering it. Approval is per-act and never generalises. |
| 2026-08-26 11:25 | **Priority was never stored, and tasks are now editable.** `Action::AddTask` carried `priority` as a `String` through `Priority::parse` — which reads the DB encoding (`HIGH`) while the UI sends serde's (`High`) — so the `unwrap_or` made *every* task `MEDIUM`. The field is typed `Priority` now, with `commands::tests` pinning the wire encoding. New `Event::EditTask` (title + priority, blank refused whole, block untouched) and `Event::AddTime` (grants only — `+5/+10/+15`, applied to the task *and* its live block, refused at a checkpoint) behind `components/TaskEditor.tsx`, used by the queue row and the current task. |
| 2026-08-26 11:05 | **Queue drag-to-reorder fixed, and the priority dot made legible.** Two independent causes for the dead drag: Tauri's `dragDropEnabled` defaults to *true* and the OS drag handler swallows HTML5 DnD (now `false` on the `main` window), and WebKit will not start a drag whose `dataTransfer` carries no payload. `queue::move_before` now reads the target's index **before** lifting the row out, so a downward drag moves. `PriorityDot` in `components/ui.tsx` is shared by the queue and the popover — which showed no priority at all — at 7px, with Low hollow. |
| 2026-08-24 19:10 | **The UI refreshes while the timer is stopped.** `timebox://changed` now also fires on window focus, and `useTimebox` polls every 10s when `timerState !== "Running"`. The tick loop is the only time-driven nudge and it parks in `IDLE`/`PAUSED`/`AWAITING_DECISION` — the exact states in which `idleMs` and `stalenessMs` grow. |
| 2026-08-24 18:10 | The tray's single break item now follows the state — *End break* during a break, disabled at a work checkpoint — rather than always reading *Take a break* and doing nothing. |
| 2026-08-24 17:45 | **Quit-parks-the-block fix.** The D16 park was hooked on `RunEvent::ExitRequested`, which `Cmd+Q` never emits — so `Cmd+Q` left the block RUNNING and the clock ran across the quit. Moved to `RunEvent::Exit`. |
| 2026-08-24 17:20 | Recorded that `tauri:dev` and the installed `/Applications/TimeBox.app` **do not share a database** — the installed build is sandboxed, so the same bundle identifier resolves to two different paths. New *Dev alongside the installed app* section under Commands. |
| 2026-08-24 16:40 | **Idle time landed** (`docs/features/IDLE_TIME.md`, D15–D22). Migration **003**; `core/model.rs` gains `IdleSpan`/`IdleReason`; `reduce` brackets every non-RUNNING interval through `sync_idle`; `core::summary` gains `idle_ms` + three causes + `outside_hours_ms`, computed as a set difference against a `window` injected by `state::window_for`. **Two invariants reversed:** quitting now *parks* the running block, and the D14 quit confirmation is deleted. New `Event::StartBreak` (D22). |
| 2026-08-21 14:30 | **Rule 17 added** — hand over at most two steps at a time, then stop and wait. |
| 2026-08-21 14:00 | The Mac App Store `.pkg` is an upload artifact and **cannot be launched locally**; `scripts/sandbox-smoketest.sh` re-signs the bundle with Developer ID + sandbox for local verification. Corrects the earlier smoke-test instruction. |
| 2026-08-21 09:35 | Startup now deletes 0.1.0's leftover `~/Library/LaunchAgents/TimeBox.plist` — it would otherwise keep launching the app behind the setting's back. |
| 2026-08-21 09:10 | Login item is **reconciled at every launch**, not only on a settings edit, and the snapshot carries `launchAtLoginActive` — what macOS actually did, versus what the user asked for. |
| 2026-08-20 22:30 | **Mac App Store prep:** `macos-private-api` and `tauri-plugin-autostart` removed. Popover corners now come from `platform/window_corners.rs`, launch-at-login from `platform/login_item.rs` (`SMAppService`). See `docs/RELEASE.md` §7. |
| 2026-08-20 14:40 | Tray icon is now the exported asset `icons/tray.png` (tauri feature `image-png` enabled to decode it), not drawn in `tray.rs`. |
| 2026-08-20 14:10 | App icon is now **exported artwork**, not script output. `icons/generate.py` is superseded and must not be re-run; regenerate with `npm run tauri icon`. Supersedes the icon gotcha dated 2026-08-19 21:52. |
| 2026-08-20 01:20 | Phase 7: added `core/summary.rs` and `db/settings.rs` to the layer map; noted migration 002 and the settings-on-the-snapshot shape. |
| 2026-08-19 21:52 | Initial version (commands, architecture, invariants, gotchas); Rules 1–15 from Gigih; Rule 16 added. |

---

## What this is

TimeBox is a native macOS menu bar utility for **task rotation timeboxing**. Its premise:

> The timer controls how long you work on a task, not whether the task is completed.

When a time block expires the app enters a blocking checkpoint and forces an explicit decision. It never silently advances. This is not a Pomodoro app — there are no fixed cycles and no automatic breaks.

## **IMPORTANT RULES:**

These rules apply to every task in this project unless explicitly overridden.
Bias: caution over speed on non-trivial work. Use judgment on trivial tasks.

### Rule 1 — Think Before Coding

State assumptions explicitly. If uncertain, ask rather than guess.
Present multiple interpretations when ambiguity exists.
Push back when a simpler approach exists.
Stop when confused. Name what's unclear.

### Rule 2 — Simplicity First

Minimum code that solves the problem. Nothing speculative.
No features beyond what was asked. No abstractions for single-use code.
Test: would a senior engineer say this is overcomplicated? If yes, simplify.

### Rule 3 — Surgical Changes

Touch only what you must. Clean up only your own mess.
Don't "improve" adjacent code, comments, or formatting.
Don't refactor what isn't broken. Match existing style.

### Rule 4 — Goal-Driven Execution

Define success criteria. Loop until verified.
Don't follow steps. Define success and iterate.
Strong success criteria let you loop independently.

### Rule 5 — Use the model only for judgment calls

Use me for: classification, drafting, summarization, extraction.
Do NOT use me for: routing, retries, deterministic transforms.
If code can answer, code answers.

### Rule 6 — Token budgets are not advisory

Per-task: 4,000 tokens. Per-session: 30,000 tokens.
If approaching budget, summarize and start fresh.
Surface the breach. Do not silently overrun.

### Rule 7 — Surface conflicts, don't average them

If two patterns contradict, pick one (more recent / more tested).
Explain why. Flag the other for cleanup.
Don't blend conflicting patterns.

### Rule 8 — Read before you write

Before adding code, read exports, immediate callers, shared utilities.
"Looks orthogonal" is dangerous. If unsure why code is structured a way, ask.

### Rule 9 — Tests verify intent, not just behavior

Tests must encode WHY behavior matters, not just WHAT it does.
A test that can't fail when business logic changes is wrong.

### Rule 10 — Checkpoint after every significant step

Summarize what was done, what's verified, what's left.
Don't continue from a state you can't describe back.
If you lose track, stop and restate.

### Rule 11 — Match the codebase's conventions, even if you disagree

Conformance > taste inside the codebase.
If you genuinely think a convention is harmful, surface it. Don't fork silently.

### Rule 12 — Fail loud

"Completed" is wrong if anything was skipped silently.
Default to surfacing uncertainty, not hiding it.

### Rule 13 - Be direct

Be direct, concise, and focused solely on the answer. Do not provide conversational filler.

### Rule 14 - Update doc when done implementing

When adding or modifying features, update related documents. Add historical timestamp (date and hour) so any change to the doc is traceable.

**The changelog goes at the TOP of the document, not the bottom. This is the default for every doc in this repo** — specs, runbooks, checklists, READMEs, gap analysis. Place it immediately after the title/intro block and before the first content section:

```markdown
# Document Title

<one-paragraph intro / status / scope block>

---

## Changelog

| Date (WIB)       | Change              |
| ---------------- | ------------------- |
| 2026-08-05 17:45 | Newest entry first. |
| 2026-08-05 14:00 | Initial version.    |

---

## 1. First real section
```

**Why:** these docs run to hundreds or thousands of lines. A changelog buried at the bottom means the reader learns what changed _after_ reading a version of the truth that may already be stale — and a stale claim near the top gets believed. Top placement makes "what moved recently, and is what I'm about to read current?" the first thing answered.

**Rules for entries:** newest first; date **and** hour; append, never rewrite history. If an entry later turns out wrong, add a new entry correcting it rather than editing the old one — but if the stale entry states something a reader would act on, also fix the _body_ text it refers to, so the two don't contradict.

### Rule 15 - Commit only when ask

Do not create a git commit unless you're asked to, refer to `skills/create_git_commit.md`
— which also governs **pushing, PRs and anything else run with Gigih's
credentials**, and forbids publishing the session link.

### Rule 16 - Don't run or build the app; hand it back

Make source changes and stop. Do not run `npm run tauri:dev` / `tauri:build`, and do not launch `TimeBox.app`, to verify UI work — you cannot see the rendered window, and a build costs ~90s for no verification.

Run what actually proves something: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `npm run typecheck`. Then hand over with a short list of what to look at. Gigih reruns `tauri:dev` and reports back.

Build or launch only when asked, or when the check is genuinely non-visual — schema written, state persisted, process CPU at idle.

### Rule 17 - Hand over at most two steps at a time

When asking Gigih to *do* something, give **at most 2 steps**, then stop and wait. He will come back and ask what's next.

A long instruction block is unfollowable — the thread gets lost and none of it gets acted on. This applies to setup walkthroughs, verification checklists, and install sequences equally.

Do not pad a reply that contains steps with optional extras, "while you're there" asides, "worth knowing" caveats, or offers of further work. Those enlarge the pile even when they are not themselves steps. Hold them until asked.

The same applies to scripts: one that prints a ten-point checklist has just moved the problem. Have him run one thing and report back.

## Commands

Rust lives in `src-tauri/`. Add `~/.cargo/bin` to `PATH` if `cargo` is not found.

```bash
npm run tauri:dev            # run the app (Vite + Rust, hot reload on the TS side)
npm run tauri:build          # bundle TimeBox.app + DMG into src-tauri/target/release/bundle/
npm run typecheck            # tsc --noEmit
npm run build                # tsc --noEmit && vite build (frontend only)

cd src-tauri
cargo test                                       # all Rust tests
cargo test t14_return_resumes_the_remainder      # a single test by name
cargo test core::tests::                         # just the domain-core suite
cargo clippy --all-targets -- -D warnings        # lint; CI-grade, must be clean
```

The fast, meaningful loop is `cargo test` + `cargo clippy` + `npm run typecheck`. A full `tauri:build` takes ~90s and proves little about UI changes.

### Dev alongside the installed app

`tauri:dev` can run while `/Applications/TimeBox.app` is installed and running.
**They do not share a database**, which is the thing to get right before
concluding anything from what one of them shows:

```
/Applications/TimeBox.app          sandboxed (com.apple.security.app-sandbox)
  → ~/Library/Containers/xyz.gigiheristiawan.timebox/Data/Library/
      Application Support/xyz.gigiheristiawan.timebox/timebox.db

npm run tauri:dev                  unsigned, no entitlements
  → ~/Library/Application Support/xyz.gigiheristiawan.timebox/timebox.db
```

Same bundle identifier both times — macOS redirects the *sandboxed* process into
its container, so `app_data_dir()` resolves to two different paths. A dev run
therefore starts on an empty database and migrates its own copy; a schema change
does not touch the real data until a signed build is installed.

To exercise a migration against real data, **copy** the container database to the
non-sandboxed path first. Never symlink it: both are WAL-mode, and the `-wal` and
`-shm` files have to travel with it.

Three things *are* shared between the two processes:

- **`Cmd+Shift+T`.** Both register it; the second to launch loses and logs
  `Cmd+Shift+T unavailable`. Whichever started first owns the shortcut.
- **The menu bar icon.** Two identical glyphs, nothing to tell them apart.
- **`tauri-plugin-single-instance`**, whose socket is keyed on the identifier.
  The sandbox stops the installed build from creating it — which is why
  `RunEvent::Reopen` exists — so dev normally launches fine. If starting dev
  instead pops the *installed* app's popover and exits, that is what happened.

## Architecture

### Rust owns every decision; TypeScript only formats

This is the single most important structural rule (`docs/SPEC.md` R6/R7).

- **`src-tauri/src/core/`** is pure: no I/O, no Tauri imports, no clock of its own. `timer_machine::reduce(state, event, now, ids) -> (MachineState, Vec<Effect>)`. The instant is injected and ids come from an `IdSource`, so the reducer is deterministic and every product rule is testable without a UI.
- **`src/core/format.ts`** contains _only_ `clockStr`, `durStr`, and `remainingMs`. No transitions, no queue mutation, no decision rules. If the UI seems to need a decision implemented client-side, that means the Rust command surface is missing a command — do not add logic here.
- The UI can only send a typed `Action` (`src-tauri/src/commands.rs`). Anything not in that enum cannot happen.
- The countdown interpolates against a backend-supplied instant plus a stored clock skew, and **never concludes expiry itself** — at 00:00 it shows zero and waits for the backend's transition.
- **How the UI learns anything changed:** one snapshot on mount, then the `timebox://changed` event. It is emitted by every dispatch, by `update_settings`, by the tray's break action, on **window focus**, and once a second by the tick loop. The tick loop is the only *time-driven* source and it parks whenever the timer is not `RUNNING`, so `useTimebox` also polls every 10s in that case. Without it `idleMs` and `stalenessMs` — the two numbers that grow precisely while the ticker is parked — sit frozen on screen until the user acts. Windows are hidden, never destroyed, so reopening one does not remount React or refetch on its own.

### Layers

```
core/           pure reducer + queue ops + model + menubar/summary  (no I/O)
                summary.rs also owns the interval algebra idle is defined on
db/             rusqlite; repo.rs snapshots whole state in one transaction;
                settings.rs reads/writes the single settings row
state.rs        App: hydrate, dispatch, the tick thread, cached settings;
                day_start_ms and window_for — the two calendar answers the
                core is handed rather than allowed to compute
platform/       checkpoint, popover, tray windows;
                login_item (SMAppService) and window_corners (rounded popover),
                both raw AppKit via objc2 — see the App Store note below
commands.rs     the entire IPC surface: get_snapshot, dispatch, update_settings,
                window plumbing, health_check
```

`Snapshot` carries `state`, `summary` (Today + capacity + idle, from
`core::summary`) and `settings` together, so the UI has one channel and no
arithmetic or second store of its own. Anything Today or the capacity strip
shows is computed in Rust.

Local midnight is resolved in `state::day_start_ms` and the working window in
`state::window_for`; both are *injected* into `core::summary`. A timezone and a
weekday are shell concerns, and the core stays pure.

There is deliberately **no SQL plugin** — the webview cannot reach the database.

### Persistence and recovery

`repo::save` writes the _whole_ state in one transaction rather than diffing. At a few hundred rows a day the cost is irrelevant, and it removes the possibility of persisting half a transition.

`App::hydrate` loads from SQLite and then feeds **exactly one `Tick`** at the current instant before anything can render. That one line is the entire recovery story — quit mid-block, crash, Mac sleep, and quit-while-awaiting-a-decision all resolve through it, and none needs its own code path. A block whose `end_at` has passed surfaces as a checkpoint, never as a running or reset timer.

The tick thread parks on a condvar, so `IDLE` / `PAUSED` / `AWAITING_DECISION` cost zero wakeups. Because it sleeps against wall time, a system wake produces a late tick that resolves expiry — which is why there is no `NSWorkspace` wake observer.

### Two windows, one bundle

`main` and `checkpoint` both load `index.html`; `src/main.tsx` routes on `getCurrentWindow().label`. Effects are applied by `platform::checkpoint::apply`, called from **both** the tick loop and the `dispatch` command, so a checkpoint reached by either path behaves identically.

## Invariants that are easy to break

Each is enforced and tested; changing one changes what the product is.

- **Block completion ≠ task completion.** A task is `Done` only via an explicit complete. (Test 9)
- **Switching parks a block, never re-grants one.** Returning to a set-down task resumes its _remainder_. Otherwise switching away at 29:00 of a 30:00 block and back would hand out a fresh 30, letting one task consume unlimited time without ever reaching a checkpoint. (Tests 14, 15 — mutation-checked)
- **At most one parked block per task**, enforced both in the reducer and by a partial unique index in the schema.
- **The checkpoint has no exit.** No dismiss/close/later/continue, no timeout, `Esc` inert, `Cmd+W` refused, and `SwitchTo`/`Pause`/`Resume`/`StartBreak` are no-ops while a work checkpoint is open.
- **`end_at` is absolute and never decremented** *while a block runs*. It is recomputed on every resume, and **quitting parks the block** — the exit path dispatches `Event::Pause`, so the interval the app is closed is idle, not work (IDLE_TIME D16, reversing SPEC §6). The anti-gaming rule is untouched: parking holds the *remainder* and never re-grants an allocation. Recorded work is still capped at the block's allocation, which is what bounds the one gap left — a crash or a sleep mid-block, until D21 lands.
- **Idle is inferred, work is observed.** `idle_ms` is *working-window time no running block covered* — a set difference over intervals, never `window − worked − break`, which goes negative when a block spans `work_end`. Outside the window, work is recorded (`outside_hours_ms`) and idle is not: no claim of presence was made there. A day with no blocks reports no idle at all (D19), which is the holiday and sick-day answer.
- **An open idle span survives a quit and keeps accruing.** Paused is still paused whether or not the app is alive (D15). This is why `repo::load` returns the open span open, and why D20's "close it at the last state write" was revised away — see `docs/features/IDLE_TIME.md`.
- **A deliberate break parks the work block and leaves the task at the queue head.** Unlike a switch, which rotates to the tail, and unlike a switch it counts no `interruptions` — D11 measures churn between tasks (D22).
- **Break blocks carry no task**, never count as worked, and do not consume daily capacity.
- **Away time is banked, not derived.** `settle_away` runs only on an *accepted* checkpoint decision. Deriving it from `end_at` afterwards would count every parked block as time waiting at a checkpoint, and banking it on a rejected event would count a single wait twice. (Tests in `core::tests`)

## Docs

- **`docs/SPEC.md`** is authoritative for the MVP. Numbered decisions `D1`–`D14` (resolved ambiguities, each with its reasoning), stack rationale `R1`–`R8` (labelled _Inherited_ vs _Chosen_), and acceptance tests 1–22. If code and spec disagree, fix one deliberately and say which. D14 is struck — see D16.
- **`docs/features/`** holds the specs that extend it, each with its own decisions and acceptance tests continuing the same numbering: `IDLE_TIME.md` (D15–D20, D22; tests 23–33, 42–47) with `IDLE_TIME_PLAN.md` beside it, and `SLEEP_DETECTION.md` (D21; tests 34–41, **not yet implemented**).
- **`docs/IMPLEMENTATION_PLAN.md`** tracks per-task status across 8 phases plus open questions. Update it as work lands.
- **`docs/RELEASE.md`** is the release runbook — icon regeneration, universal build, signing, notarization, and the performance check, with the exact commands.
- **`docs/mockup.html`** is the interactive design reference — the real product logic in a single HTML file. Useful for checking intended interaction before building a component.
- Rust test names match spec test numbers (`t14_return_resumes_the_remainder`), so a failure names its requirement.

### No private API — the App Store constraint

The app is built to be shippable to the Mac App Store as well as by DMG, so
**nothing may use a private API and nothing may write outside the sandbox
container.** Two places would have, and were deliberately replaced:

- `platform/window_corners.rs` rounds the popover through `setOpaque:NO` plus a
  corner radius on the content view's layer. Do **not** reintroduce
  `transparent(true)` / the `macos-private-api` feature to get the same effect.
- `platform/login_item.rs` registers the login item with `SMAppService` rather
  than `tauri-plugin-autostart`, which writes into `~/Library/LaunchAgents`.
  `SMAppService.mainApp` *aborts the process* when the binary is not in a
  bundle, so the module guards on `NSBundle.mainBundle.bundleIdentifier`; that
  guard is what makes `tauri:dev` survive the toggle.

  Unlike the plist it replaced, `SMAppService` can **refuse** — it wants the app
  signed and in `/Applications`. So `reconcile` runs on every launch rather than
  only when the setting changes (a user who toggles it on from `~/Downloads` and
  later moves the app would otherwise never be registered), and the snapshot
  carries `launchAtLoginActive` so the toggle shows the system's answer instead
  of the stored wish. `is_active()` reads a cache; `status` crosses an XPC
  boundary and the snapshot is rebuilt every second.

  Startup also runs `remove_legacy_launch_agent()`: 0.1.0 shipped
  `tauri-plugin-autostart`, which wrote `~/Library/LaunchAgents/TimeBox.plist`,
  and nothing else would ever remove it. Left there it launches the app at login
  independently of the setting, so switching the toggle *off* would unregister
  the service and still start the app. It deletes only when the plist's
  `ProgramArguments` points into a `TimeBox.app` — a filename match is not
  evidence enough to delete a file out of the user's `LaunchAgents`.

`tauri-plugin-single-instance` stays for the DMG build even though its `/tmp`
socket is sandbox-illegal; `RunEvent::Reopen` in `lib.rs` covers the same
"second launch shows the popover" behaviour where the plugin cannot.

## Gotchas

- Migrations store **milliseconds**, matching the core exactly; second-granularity would accumulate rounding into real drift. `001` was rewritten in place pre-release; `002` (`away_ms`, `first_run_done`) and `003` (the working window and `idle_spans`) followed the forward-only rule. From here, add a `004`.
- `available_work_minutes_per_day`, `work_start_minutes` and `work_end_minutes` are the columns in **minutes** — `db/settings.rs` converts at the boundary so the rest of the app stays in milliseconds. The two window columns are a wall-clock *time of day*, not a duration: `state::window_for` resolves them against a local date, so a DST day's window still starts at 09:00 by the clock on the wall.
- **`Cmd+Q` does not emit `RunEvent::ExitRequested`.** It is muda's predefined Quit item, which sends `terminate:` straight to `NSApplication`; tao sees `applicationWillTerminate` and emits **`Exit` only**. Anything that must happen before the process dies — the D16 park in `lib.rs` is the one that matters — belongs on `RunEvent::Exit`, which every quit path reaches including `handle.exit(0)`. Hooked on `ExitRequested` it fails silently, and only from `Cmd+Q`: the popover's Quit item still worked, which is what made it look fine.
- `idle_spans` has a partial unique index on the open row, so `repo::save` writes the closed spans **before** the open one — inserting a newly opened span before the previous one's `ended_at` lands would be rejected.
- **HTML5 drag-and-drop needs `"dragDropEnabled": false`** on the window (`tauri.conf.json`). Left at its default, macOS's own drag-drop handler on the WKWebView eats `dragstart`/`drop` and the queue rows look inert with nothing logged. WebKit also drops any drag whose `dragstart` did not call `dataTransfer.setData` — both had to be true at once for a row to move.
- If `tauri:build` fails in `bundle_dmg.sh`, an interrupted earlier build left a disk image mounted: `hdiutil detach /Volumes/dmg.* -force` and delete `src-tauri/target/release/bundle/macos/rw.*.dmg`.
- `grep -r TEMPORARY src/` — 1-minute test durations exist to make the checkpoint reachable quickly. Remove before release (plan task 8.0).
- The app icon is **exported artwork**, regenerated from a 1024px master with `npm run tauri icon -- <master>.png`. `src-tauri/icons/generate.py` drew the earlier icon in code and is **superseded — do not run it**, it overwrites the exported set. See `docs/RELEASE.md` §1 for the export geometry. The **tray** icon is a separate asset, `icons/tray.png`, compiled into the binary by `platform/tray.rs` — `tauri icon` does not generate it. It must be a _template_ image (pure black + alpha, cutouts as real transparency) to work in both menu bar themes, so it cannot reuse the coloured artwork; re-export it whenever the mark changes. Spec in `docs/RELEASE.md` §1.
