# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Changelog

| Date (WIB)       | Change                                                                                               |
| ---------------- | ---------------------------------------------------------------------------------------------------- |
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

Do not create a git commit unless you're asked to, refer to `skills/create_git_commit.md`.

### Rule 16 - Don't run or build the app; hand it back

Make source changes and stop. Do not run `npm run tauri:dev` / `tauri:build`, and do not launch `TimeBox.app`, to verify UI work — you cannot see the rendered window, and a build costs ~90s for no verification.

Run what actually proves something: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `npm run typecheck`. Then hand over with a short list of what to look at. Gigih reruns `tauri:dev` and reports back.

Build or launch only when asked, or when the check is genuinely non-visual — schema written, state persisted, process CPU at idle.

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

## Architecture

### Rust owns every decision; TypeScript only formats

This is the single most important structural rule (`docs/SPEC.md` R6/R7).

- **`src-tauri/src/core/`** is pure: no I/O, no Tauri imports, no clock of its own. `timer_machine::reduce(state, event, now, ids) -> (MachineState, Vec<Effect>)`. The instant is injected and ids come from an `IdSource`, so the reducer is deterministic and every product rule is testable without a UI.
- **`src/core/format.ts`** contains _only_ `clockStr`, `durStr`, and `remainingMs`. No transitions, no queue mutation, no decision rules. If the UI seems to need a decision implemented client-side, that means the Rust command surface is missing a command — do not add logic here.
- The UI can only send a typed `Action` (`src-tauri/src/commands.rs`). Anything not in that enum cannot happen.
- The countdown interpolates against a backend-supplied instant plus a stored clock skew, and **never concludes expiry itself** — at 00:00 it shows zero and waits for the backend's transition.

### Layers

```
core/           pure reducer + queue ops + model + menubar/summary  (no I/O)
db/             rusqlite; repo.rs snapshots whole state in one transaction;
                settings.rs reads/writes the single settings row
state.rs        App: hydrate, dispatch, the tick thread, cached settings
platform/       checkpoint, popover, tray, quit-confirm windows
commands.rs     the entire IPC surface: get_snapshot, dispatch, update_settings,
                window plumbing, health_check
```

`Snapshot` carries `state`, `summary` (Today + capacity, from `core::summary`) and
`settings` together, so the UI has one channel and no arithmetic or second store
of its own. Anything Today or the capacity strip shows is computed in Rust.

Local midnight is resolved in `state::day_start_ms` and *injected* into
`core::summary` — a timezone is a shell concern, and the core stays pure.

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
- **The checkpoint has no exit.** No dismiss/close/later/continue, no timeout, `Esc` inert, `Cmd+W` refused, and `SwitchTo`/`Pause`/`Resume` are no-ops while a work checkpoint is open.
- **`end_at` is absolute and never decremented.** Quitting does not stop the clock — only an explicit `pause()` holds a remainder. Recorded work is capped at the block's allocation.
- **Break blocks carry no task**, never count as worked, and do not consume daily capacity.
- **Away time is banked, not derived.** `settle_away` runs only on an *accepted* checkpoint decision. Deriving it from `end_at` afterwards would count every parked block as time waiting at a checkpoint, and banking it on a rejected event would count a single wait twice. (Tests in `core::tests`)

## Docs

- **`docs/SPEC.md`** is authoritative. Numbered decisions `D1`–`D14` (resolved ambiguities, each with its reasoning), stack rationale `R1`–`R8` (labelled _Inherited_ vs _Chosen_), and acceptance tests 1–22. If code and spec disagree, fix one deliberately and say which.
- **`docs/IMPLEMENTATION_PLAN.md`** tracks per-task status across 8 phases plus open questions. Update it as work lands.
- **`docs/RELEASE.md`** is the release runbook — icon regeneration, universal build, signing, notarization, and the performance check, with the exact commands.
- **`docs/mockup.html`** is the interactive design reference — the real product logic in a single HTML file. Useful for checking intended interaction before building a component.
- Rust test names match spec test numbers (`t14_return_resumes_the_remainder`), so a failure names its requirement.

## Gotchas

- Migrations store **milliseconds**, matching the core exactly; second-granularity would accumulate rounding into real drift. `001` was rewritten in place pre-release; `002` (`away_ms`, `first_run_done`) followed the forward-only rule. From here, add a `003`.
- `settings.available_work_minutes_per_day` is the one column in **minutes** — `db/settings.rs` converts at the boundary so the rest of the app stays in milliseconds.
- If `tauri:build` fails in `bundle_dmg.sh`, an interrupted earlier build left a disk image mounted: `hdiutil detach /Volumes/dmg.* -force` and delete `src-tauri/target/release/bundle/macos/rw.*.dmg`.
- `grep -r TEMPORARY src/` — 1-minute test durations exist to make the checkpoint reachable quickly. Remove before release (plan task 8.0).
- The app icon is **exported artwork**, regenerated from a 1024px master with `npm run tauri icon -- <master>.png`. `src-tauri/icons/generate.py` drew the earlier icon in code and is **superseded — do not run it**, it overwrites the exported set. See `docs/RELEASE.md` §1 for the export geometry. The **tray** icon is a separate asset, `icons/tray.png`, compiled into the binary by `platform/tray.rs` — `tauri icon` does not generate it. It must be a _template_ image (pure black + alpha, cutouts as real transparency) to work in both menu bar themes, so it cannot reuse the coloured artwork; re-export it whenever the mark changes. Spec in `docs/RELEASE.md` §1.
