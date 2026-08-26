//! The timer state machine.
//!
//! `reduce` is total and side-effect free: it takes a state, an event, and the
//! current instant, and returns a new state plus effects for the shell to
//! perform. Invalid events are no-ops rather than panics.

use super::model::*;
use super::queue;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineState {
    pub timer_state: TimerState,
    pub tasks: Vec<Task>,
    pub blocks: Vec<TimeBlock>,
    pub queue: Vec<TaskId>,
    pub current_block_id: Option<BlockId>,

    /// Closed idle spans, in the order they were banked. Not sent to the UI:
    /// the numbers it needs are already on the snapshot's summary, and the
    /// list grows by a handful of rows a day forever.
    #[serde(skip)]
    pub idle_spans: Vec<IdleSpan>,
    /// The span currently accruing. `Some` exactly when the timer is not
    /// RUNNING *and* something has run at least once — at most one, mirroring
    /// `current_block_id` and the schema's partial unique index.
    #[serde(skip)]
    pub open_idle: Option<IdleSpan>,
}

impl Default for MachineState {
    fn default() -> Self {
        Self {
            timer_state: TimerState::Idle,
            tasks: Vec::new(),
            blocks: Vec::new(),
            queue: Vec::new(),
            current_block_id: None,
            idle_spans: Vec::new(),
            open_idle: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Start or resume a specific task. Valid from every state except a work
    /// checkpoint — the checkpoint has no side doors (SPEC §5.2).
    SwitchTo { task: TaskId },
    Pause,
    Resume,
    /// End this block early, keep the task, rotate it back (SPEC D2).
    Skip,
    /// Finish the task now, regardless of time left (acceptance test 8).
    CompleteCurrentTask,
    /// Re-evaluate expiry. Emitted by the tick loop, on wake, and on launch.
    Tick,

    DecideComplete,
    DecidePending,
    DecideExtend { ms: Millis },
    /// Break is a modifier on the task decision, never a substitute (SPEC D7).
    DecideBreak { ms: Millis, complete: bool },
    EndBreak,
    ExtendBreak { ms: Millis },
    /// Start a break without waiting for a checkpoint (IDLE_TIME D22). Parks
    /// the work block and leaves the task at the queue *head* — a break is a
    /// return to the same work, not a rotation away from it.
    StartBreak { ms: Millis },

    RemoveTask { task: TaskId },
    /// Adding never starts anything — the user chooses when work begins.
    AddTask { title: String, block_ms: Millis, priority: Priority },
    /// Rename and re-prioritise. Never touches the block: a task's allocation
    /// is the timer's business, and editing one mid-block must not re-grant
    /// time the way a fresh block would.
    EditTask { task: TaskId, title: String, priority: Priority },
    /// Grant a task more time: `ms` is *added*, never assigned. There is no way
    /// to shorten an allocation from here — trimming a running block would let
    /// a checkpoint be reached early and dodged, and trimming a parked one
    /// would rewrite time already promised.
    AddTime { task: TaskId, ms: Millis },
    /// Drag-and-drop reorder: `moved` takes the place `before` holds.
    Reorder { moved: TaskId, before: TaskId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Persist,
    EnterCheckpoint { block: BlockId, kind: BlockKind },
    LeaveCheckpoint,
    PlayExpirySound,
    Notify { kind: BlockKind, task_title: Option<String>, allocated_minutes: i64 },
    StartTicking,
    StopTicking,
    UpdateMenuBar,
}

// ---------------------------------------------------------------- accessors

impl MachineState {
    pub fn current_block(&self) -> Option<&TimeBlock> {
        let id = self.current_block_id.as_ref()?;
        self.blocks.iter().find(|b| &b.id == id)
    }

    pub fn current_task(&self) -> Option<&Task> {
        let tid = self.current_block()?.task_id.as_ref()?;
        self.tasks.iter().find(|t| &t.id == tid)
    }

    pub fn on_break(&self) -> bool {
        matches!(self.current_block(), Some(b) if b.kind == BlockKind::Break)
    }

    pub fn remaining_ms(&self, now: Millis) -> Millis {
        self.current_block().map_or(0, |b| b.remaining_ms(now))
    }

    /// The parked block holding a task's remainder, if any (SPEC D10).
    /// At most one can exist per task; the schema enforces this too.
    pub fn parked_for(&self, task: &TaskId) -> Option<&TimeBlock> {
        self.blocks.iter().find(|b| {
            b.task_id.as_ref() == Some(task) && b.is_parked(self.current_block_id.as_ref())
        })
    }

    /// How long an unanswered checkpoint has been waiting (SPEC D13).
    /// `None` unless a checkpoint is actually open.
    pub fn staleness_ms(&self, now: Millis) -> Option<Millis> {
        if self.timer_state != TimerState::AwaitingDecision {
            return None;
        }
        let expired_at = self.current_block()?.end_at?;
        Some((now - expired_at).max(0))
    }

    fn task_mut(&mut self, id: &TaskId) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| &t.id == id)
    }

    fn block_mut(&mut self, id: &BlockId) -> Option<&mut TimeBlock> {
        self.blocks.iter_mut().find(|b| &b.id == id)
    }
}

// ------------------------------------------------------------------ reducer

pub fn reduce(
    mut state: MachineState,
    event: Event,
    now: Millis,
    ids: &mut dyn IdSource,
) -> (MachineState, Vec<Effect>) {
    let mut fx = Vec::new();
    let before = state.timer_state;

    match event {
        Event::Tick => {
            if state.timer_state == TimerState::Running {
                let expired = state
                    .current_block()
                    .and_then(|b| b.end_at)
                    .is_some_and(|end| now >= end);
                if expired {
                    expire(&mut state, now, &mut fx);
                }
            }
        }

        Event::SwitchTo { task } => switch_to(&mut state, &task, now, ids, &mut fx),

        Event::Pause => {
            if state.timer_state == TimerState::Running {
                if let Some(id) = state.current_block_id.clone() {
                    park(&mut state, &id, now);
                    state.timer_state = TimerState::Paused;
                    fx.push(Effect::StopTicking);
                }
            }
        }

        Event::Resume => {
            if state.timer_state == TimerState::Paused {
                if let Some(id) = state.current_block_id.clone() {
                    unpark(&mut state, &id, now);
                    state.timer_state = TimerState::Running;
                    fx.push(Effect::StartTicking);
                }
            }
        }

        Event::Skip => {
            if let Some(b) = state.current_block().cloned() {
                if b.kind == BlockKind::Break {
                    end_current(&mut state, BlockStatus::Completed, now);
                } else {
                    end_current(&mut state, BlockStatus::Skipped, now);
                    if let Some(t) = b.task_id.clone() {
                        queue::rotate_to_back(&mut state.queue, &t);
                    }
                }
                start_next(&mut state, now, ids, &mut fx);
            }
        }

        Event::CompleteCurrentTask => {
            if let Some(b) = state.current_block().cloned() {
                if let Some(t) = b.task_id.clone() {
                    end_current(&mut state, BlockStatus::Completed, now);
                    finish_task(&mut state, &t, now);
                    start_next(&mut state, now, ids, &mut fx);
                }
            }
        }

        Event::DecideComplete => {
            if at_work_checkpoint(&state) {
                settle_away(&mut state, now);
                let t = state.current_task().map(|t| t.id.clone());
                end_current(&mut state, BlockStatus::Completed, now);
                if let Some(t) = t {
                    finish_task(&mut state, &t, now);
                }
                fx.push(Effect::LeaveCheckpoint);
                start_next(&mut state, now, ids, &mut fx);
            }
        }

        Event::DecidePending => {
            if at_work_checkpoint(&state) {
                settle_away(&mut state, now);
                let t = state.current_task().map(|t| t.id.clone());
                end_current(&mut state, BlockStatus::Completed, now);
                if let Some(t) = t {
                    queue::rotate_to_back(&mut state.queue, &t);
                }
                fx.push(Effect::LeaveCheckpoint);
                start_next(&mut state, now, ids, &mut fx);
            }
        }

        Event::DecideExtend { ms } => {
            if at_work_checkpoint(&state) && ms > 0 {
                settle_away(&mut state, now);
                if let Some(id) = state.current_block_id.clone() {
                    if let Some(b) = state.block_mut(&id) {
                        b.extension_ms += ms;
                        b.end_at = Some(now + ms);
                        b.last_resume_at = Some(now);
                        b.status = BlockStatus::Running;
                    }
                    state.timer_state = TimerState::Running;
                    fx.push(Effect::LeaveCheckpoint);
                    fx.push(Effect::StartTicking);
                }
            }
        }

        Event::DecideBreak { ms, complete } => {
            if at_work_checkpoint(&state) && ms > 0 {
                settle_away(&mut state, now);
                let t = state.current_task().map(|t| t.id.clone());
                end_current(&mut state, BlockStatus::Completed, now);
                if let Some(t) = t {
                    if complete {
                        finish_task(&mut state, &t, now);
                    } else {
                        queue::rotate_to_back(&mut state.queue, &t);
                    }
                }
                fx.push(Effect::LeaveCheckpoint);
                start_break(&mut state, ms, now, ids, &mut fx);
            }
        }

        Event::EndBreak => {
            if at_break_checkpoint(&state) || state.on_break() {
                settle_away(&mut state, now);
                end_current(&mut state, BlockStatus::Completed, now);
                fx.push(Effect::LeaveCheckpoint);
                start_next(&mut state, now, ids, &mut fx);
            }
        }

        Event::ExtendBreak { ms } => {
            if state.on_break() && ms > 0 {
                settle_away(&mut state, now);
                if let Some(id) = state.current_block_id.clone() {
                    if let Some(b) = state.block_mut(&id) {
                        b.extension_ms += ms;
                        b.end_at = Some(now + ms);
                        b.last_resume_at = Some(now);
                        b.status = BlockStatus::Running;
                    }
                    state.timer_state = TimerState::Running;
                    fx.push(Effect::LeaveCheckpoint);
                    fx.push(Effect::StartTicking);
                }
            }
        }

        Event::StartBreak { ms } => {
            // The checkpoint has no side doors, and during a break the
            // operation is ExtendBreak (tests 44, 45).
            if ms > 0 && !at_work_checkpoint(&state) && !state.on_break() {
                if let Some(id) = state.current_block_id.clone() {
                    if state.timer_state == TimerState::Running {
                        // Parked, not ended, and with no interruption counted:
                        // D11 measures churn between tasks, and tinting Today
                        // because someone took lunch would contradict D9.
                        park(&mut state, &id, now);
                    }
                    state.current_block_id = None;
                }
                start_break(&mut state, ms, now, ids, &mut fx);
            }
        }

        Event::AddTask { title, block_ms, priority } => {
            let title = title.trim().to_string();
            if !title.is_empty() && block_ms > 0 {
                let mut t = Task::new(ids.next_id(), title, block_ms, now);
                t.priority = priority;
                state.queue.push(t.id.clone());
                state.tasks.push(t);
            }
        }

        Event::EditTask { task, title, priority } => {
            let title = title.trim().to_string();
            // A blank title is rejected outright rather than half-applied:
            // AddTask refuses one, so an edit cannot be the way to get one.
            if !title.is_empty() {
                if let Some(t) = state.task_mut(&task) {
                    t.title = title;
                    t.priority = priority;
                }
            }
        }

        Event::AddTime { task, ms } => {
            if ms > 0 {
                if let Some(t) = state.task_mut(&task) {
                    t.block_duration_ms += ms;
                }
                // The live block, if this task holds one, so the grant applies
                // now rather than only to the task's next block. A checkpoint is
                // excluded: that block belongs to the decision, and `Extend` is
                // how the checkpoint grants time — this must not be a way around
                // answering it.
                let live = if state.timer_state == TimerState::AwaitingDecision {
                    None
                } else {
                    state
                        .current_block()
                        .filter(|b| b.task_id.as_ref() == Some(&task))
                        .map(|b| b.id.clone())
                        .or_else(|| state.parked_for(&task).map(|b| b.id.clone()))
                };
                if let Some(id) = live {
                    if let Some(b) = state.block_mut(&id) {
                        b.extension_ms += ms;
                        if b.status == BlockStatus::Running {
                            // Pushed forward, never recomputed from `now`: the
                            // time already burnt stays burnt.
                            b.end_at = b.end_at.map(|e| e + ms);
                        } else if let Some(left) = b.remaining_when_paused_ms {
                            b.remaining_when_paused_ms = Some(left + ms);
                        }
                    }
                }
            }
        }

        Event::Reorder { moved, before } => {
            queue::move_before(&mut state.queue, &moved, &before);
        }

        Event::RemoveTask { task } => {
            queue::remove(&mut state.queue, &task);
            // A parked block for a removed task would otherwise linger and be
            // resumed by a task that no longer exists.
            let parked: Vec<BlockId> = state
                .blocks
                .iter()
                .filter(|b| b.task_id.as_ref() == Some(&task) && b.is_parked(state.current_block_id.as_ref()))
                .map(|b| b.id.clone())
                .collect();
            for id in parked {
                if let Some(b) = state.block_mut(&id) {
                    b.status = BlockStatus::Cancelled;
                    b.ended_at = Some(now);
                }
            }
            if let Some(t) = state.task_mut(&task) {
                t.status = TaskStatus::Cancelled;
            }
            // If it was running, stop cleanly rather than pointing at a ghost.
            let was_current = state
                .current_block()
                .and_then(|b| b.task_id.clone())
                .is_some_and(|t| t == task);
            if was_current {
                settle_away(&mut state, now);
                end_current(&mut state, BlockStatus::Cancelled, now);
                start_next(&mut state, now, ids, &mut fx);
            }
        }
    }

    sync_idle(&mut state, before, now, ids);

    fx.push(Effect::UpdateMenuBar);
    fx.push(Effect::Persist);
    (state, fx)
}

// ----------------------------------------------------------------- internals

fn at_work_checkpoint(s: &MachineState) -> bool {
    s.timer_state == TimerState::AwaitingDecision && !s.on_break()
}

fn at_break_checkpoint(s: &MachineState) -> bool {
    s.timer_state == TimerState::AwaitingDecision && s.on_break()
}

/// Bracket every instant the timer is not running (IDLE_TIME §5.4).
///
/// Called once, from the end of `reduce`, rather than per event arm — the rule
/// is about the state entered, not about which event got there, and duplicating
/// it per arm is how the two would drift.
///
/// The span closed here and `away_ms` record the same interval when the state
/// left is `AwaitingDecision`. They are two views of one fact, not two
/// accumulators: `away_ms` is per block (D13, and what the staleness line
/// reads), the span is per interval (what the day's idle reads). Neither is
/// derived from the other, and both are written only on a real transition, so
/// they agree.
fn sync_idle(state: &mut MachineState, before: TimerState, now: Millis, ids: &mut dyn IdSource) {
    let after = state.timer_state;
    if after == before {
        return;
    }
    if let Some(mut open) = state.open_idle.take() {
        // A zero-length span is banked as nothing rather than as a row.
        if now > open.started_at {
            open.ended_at = Some(now);
            state.idle_spans.push(open);
        }
    }
    if let Some(reason) = IdleReason::of(after) {
        state.open_idle = Some(IdleSpan {
            id: ids.next_id(),
            started_at: now,
            ended_at: None,
            reason,
        });
    }
}

/// Bank the time the current block spent waiting for an answer (SPEC D13).
///
/// Called from every path that answers a checkpoint, and only from those — a
/// rejected event must not bank a gap that is still running. Extending re-arms
/// the block, so this accumulates rather than assigns.
fn settle_away(state: &mut MachineState, now: Millis) {
    if state.timer_state != TimerState::AwaitingDecision {
        return;
    }
    let Some(id) = state.current_block_id.clone() else { return };
    if let Some(b) = state.block_mut(&id) {
        let expired_at = b.end_at.unwrap_or(now);
        b.away_ms += (now - expired_at).max(0);
    }
}

/// Hold a block's remainder. Used by both `Pause` and by parking on a switch —
/// the arithmetic is identical; only the queue treatment differs.
fn park(state: &mut MachineState, id: &BlockId, now: Millis) {
    if let Some(b) = state.block_mut(id) {
        b.accumulated_active_ms = b.active_ms(now);
        b.remaining_when_paused_ms = Some(b.remaining_ms(now));
        b.last_resume_at = None;
        b.paused_at = Some(now);
        b.status = BlockStatus::Paused;
    }
}

fn unpark(state: &mut MachineState, id: &BlockId, now: Millis) {
    if let Some(b) = state.block_mut(id) {
        let left = b.remaining_when_paused_ms.unwrap_or(0);
        b.end_at = Some(now + left);
        b.last_resume_at = Some(now);
        b.paused_at = None;
        b.status = BlockStatus::Running;
    }
}

fn end_current(state: &mut MachineState, status: BlockStatus, now: Millis) {
    if let Some(id) = state.current_block_id.clone() {
        if let Some(b) = state.block_mut(&id) {
            // Capped, so a block reopened days later cannot report days of work.
            let cap = b.alloc_ms();
            b.actual_ms = Some(b.active_ms(now).min(cap));
            b.ended_at = Some(now);
            b.status = status;
        }
    }
    state.current_block_id = None;
}

fn finish_task(state: &mut MachineState, id: &TaskId, now: Millis) {
    if let Some(t) = state.task_mut(id) {
        t.status = TaskStatus::Done;
        t.completed_at = Some(now);
    }
    queue::remove(&mut state.queue, id);
}

fn expire(state: &mut MachineState, now: Millis, fx: &mut Vec<Effect>) {
    let Some(id) = state.current_block_id.clone() else { return };
    let (kind, minutes) = {
        let Some(b) = state.block_mut(&id) else { return };
        b.accumulated_active_ms = b.active_ms(now);
        b.last_resume_at = None;
        b.status = BlockStatus::AwaitingDecision;
        (b.kind, b.alloc_ms() / 60_000)
    };
    state.timer_state = TimerState::AwaitingDecision;

    let title = state.current_task().map(|t| t.title.clone());
    fx.push(Effect::StopTicking);
    fx.push(Effect::EnterCheckpoint { block: id, kind });
    fx.push(Effect::PlayExpirySound);
    fx.push(Effect::Notify { kind, task_title: title, allocated_minutes: minutes });
}

fn start_break(
    state: &mut MachineState,
    ms: Millis,
    now: Millis,
    ids: &mut dyn IdSource,
    fx: &mut Vec<Effect>,
) {
    let b = TimeBlock {
        id: ids.next_id(),
        kind: BlockKind::Break,
        task_id: None,
        planned_ms: ms,
        extension_ms: 0,
        interruptions: 0,
        actual_ms: None,
        status: BlockStatus::Running,
        started_at: Some(now),
        ended_at: None,
        end_at: Some(now + ms),
        remaining_when_paused_ms: None,
        accumulated_active_ms: 0,
        last_resume_at: Some(now),
        paused_at: None,
        away_ms: 0,
    };
    state.current_block_id = Some(b.id.clone());
    state.blocks.push(b);
    state.timer_state = TimerState::Running;
    fx.push(Effect::StartTicking);
}

fn start_next(state: &mut MachineState, now: Millis, ids: &mut dyn IdSource, fx: &mut Vec<Effect>) {
    match queue::head(&state.queue).cloned() {
        Some(t) => start_task(state, &t, now, ids, fx),
        None => {
            state.timer_state = TimerState::Idle;
            state.current_block_id = None;
            fx.push(Effect::StopTicking);
        }
    }
}

/// Start a task — resuming its parked block if it has one. A parked block is
/// never replaced by a fresh allocation; that would make switching a way to
/// farm unlimited time (SPEC D10, acceptance tests 14 and 15).
fn start_task(
    state: &mut MachineState,
    task: &TaskId,
    now: Millis,
    ids: &mut dyn IdSource,
    fx: &mut Vec<Effect>,
) {
    if !state.tasks.iter().any(|t| &t.id == task) {
        return;
    }
    queue::bring_to_front(&mut state.queue, task);
    if let Some(t) = state.task_mut(task) {
        if t.status == TaskStatus::Todo {
            t.status = TaskStatus::InProgress;
        }
    }

    if let Some(parked) = state.parked_for(task).map(|b| (b.id.clone(), b.remaining_when_paused_ms.unwrap_or(0))) {
        let (id, left) = parked;
        state.current_block_id = Some(id.clone());
        if left <= RESUME_FLOOR_MS {
            // Nothing meaningful is left; the allocation is spent either way.
            let kind = state.block_mut(&id).map(|b| {
                b.status = BlockStatus::AwaitingDecision;
                b.end_at = Some(now);
                b.kind
            });
            state.timer_state = TimerState::AwaitingDecision;
            if let Some(kind) = kind {
                let title = state.current_task().map(|t| t.title.clone());
                let minutes = state.current_block().map_or(0, |b| b.alloc_ms() / 60_000);
                fx.push(Effect::EnterCheckpoint { block: id, kind });
                fx.push(Effect::Notify { kind, task_title: title, allocated_minutes: minutes });
            }
        } else {
            unpark(state, &id, now);
            state.timer_state = TimerState::Running;
            fx.push(Effect::StartTicking);
        }
        return;
    }

    let planned = state
        .tasks
        .iter()
        .find(|t| &t.id == task)
        .map_or(0, |t| t.block_duration_ms);
    let b = TimeBlock {
        id: ids.next_id(),
        kind: BlockKind::Work,
        task_id: Some(task.clone()),
        planned_ms: planned,
        extension_ms: 0,
        interruptions: 0,
        actual_ms: None,
        status: BlockStatus::Running,
        started_at: Some(now),
        ended_at: None,
        end_at: Some(now + planned),
        remaining_when_paused_ms: None,
        accumulated_active_ms: 0,
        last_resume_at: Some(now),
        paused_at: None,
        away_ms: 0,
    };
    state.current_block_id = Some(b.id.clone());
    state.blocks.push(b);
    state.timer_state = TimerState::Running;
    fx.push(Effect::StartTicking);
}

fn switch_to(
    state: &mut MachineState,
    task: &TaskId,
    now: Millis,
    ids: &mut dyn IdSource,
    fx: &mut Vec<Effect>,
) {
    // A work checkpoint must be answered; switching is not an escape hatch.
    if at_work_checkpoint(state) {
        return;
    }
    if state.current_block().and_then(|b| b.task_id.clone()).as_ref() == Some(task) {
        return;
    }

    if let Some(b) = state.current_block().cloned() {
        if b.kind == BlockKind::Break {
            // Cutting a break short is not an interruption of work.
            settle_away(state, now);
            end_current(state, BlockStatus::Completed, now);
            fx.push(Effect::LeaveCheckpoint);
        } else {
            park(state, &b.id, now);
            if let Some(blk) = state.block_mut(&b.id) {
                blk.interruptions += 1;
            }
            if let Some(t) = b.task_id.clone() {
                queue::rotate_to_back(&mut state.queue, &t);
            }
            state.current_block_id = None;
        }
    }

    start_task(state, task, now, ids, fx);
}
