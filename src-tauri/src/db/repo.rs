//! Snapshot persistence for the machine state.
//!
//! The reducer returns a whole state, so the repository writes a whole state,
//! in one transaction. At a few hundred rows a day the cost is irrelevant and
//! it removes an entire class of bug: there is no diff to get wrong, and no
//! way to persist half a transition (SPEC §4.5, acceptance test 7).

use crate::core::model::*;
use crate::core::timer_machine::MachineState;
use rusqlite::{params, Connection, OptionalExtension};

pub fn save(conn: &mut Connection, state: &MachineState, now: Millis) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;

    // Queue order is the queue_position column; anything not queued is NULL.
    let pos_of = |id: &TaskId| state.queue.iter().position(|q| q == id).map(|i| i as i64);

    tx.execute("DELETE FROM tasks WHERE id NOT IN (SELECT id FROM tasks)", [])?;
    for t in &state.tasks {
        tx.execute(
            "INSERT INTO tasks (id, title, status, priority, block_duration_ms,
                                queue_position, created_at, completed_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET
                title=excluded.title, status=excluded.status, priority=excluded.priority,
                block_duration_ms=excluded.block_duration_ms,
                queue_position=excluded.queue_position, completed_at=excluded.completed_at",
            params![
                t.id, t.title, t.status.as_str(), t.priority.as_str(),
                t.block_duration_ms, pos_of(&t.id), t.created_at, t.completed_at
            ],
        )?;
    }

    // current_block_id references time_blocks, so clear it before writing blocks
    // and set it after — otherwise a not-yet-inserted block trips the FK.
    tx.execute("UPDATE app_state SET current_block_id = NULL WHERE id = 1", [])?;

    for b in &state.blocks {
        tx.execute(
            "INSERT INTO time_blocks (id, kind, task_id, planned_ms, extension_ms, interruptions,
                                      actual_ms, status, started_at, ended_at, end_at, paused_at,
                                      remaining_when_paused_ms, accumulated_active_ms, last_resume_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
             ON CONFLICT(id) DO UPDATE SET
                extension_ms=excluded.extension_ms, interruptions=excluded.interruptions,
                actual_ms=excluded.actual_ms, status=excluded.status,
                ended_at=excluded.ended_at, end_at=excluded.end_at, paused_at=excluded.paused_at,
                remaining_when_paused_ms=excluded.remaining_when_paused_ms,
                accumulated_active_ms=excluded.accumulated_active_ms,
                last_resume_at=excluded.last_resume_at",
            params![
                b.id, b.kind.as_str(), b.task_id, b.planned_ms, b.extension_ms, b.interruptions,
                b.actual_ms, b.status.as_str(), b.started_at, b.ended_at, b.end_at, b.paused_at,
                b.remaining_when_paused_ms, b.accumulated_active_ms, b.last_resume_at
            ],
        )?;
    }

    tx.execute(
        "UPDATE app_state SET timer_state=?1, current_block_id=?2, updated_at=?3 WHERE id=1",
        params![state.timer_state.as_str(), state.current_block_id, now],
    )?;

    tx.commit()
}

pub fn load(conn: &Connection) -> rusqlite::Result<MachineState> {
    let mut tasks = Vec::new();
    let mut queue: Vec<(i64, TaskId)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, title, status, priority, block_duration_ms, queue_position,
                    created_at, completed_at
             FROM tasks ORDER BY queue_position IS NULL, queue_position, created_at",
        )?;
        let rows = stmt.query_map([], |r| {
            let id: String = r.get(0)?;
            let status: String = r.get(2)?;
            let priority: String = r.get(3)?;
            let pos: Option<i64> = r.get(5)?;
            Ok((
                Task {
                    id: id.clone(),
                    title: r.get(1)?,
                    status: TaskStatus::parse(&status).unwrap_or(TaskStatus::Todo),
                    priority: Priority::parse(&priority).unwrap_or(Priority::Medium),
                    block_duration_ms: r.get(4)?,
                    created_at: r.get(6)?,
                    completed_at: r.get(7)?,
                },
                pos,
            ))
        })?;
        for row in rows {
            let (t, pos) = row?;
            if let Some(p) = pos {
                queue.push((p, t.id.clone()));
            }
            tasks.push(t);
        }
    }
    queue.sort_by_key(|(p, _)| *p);

    let mut blocks = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, kind, task_id, planned_ms, extension_ms, interruptions, actual_ms,
                    status, started_at, ended_at, end_at, paused_at,
                    remaining_when_paused_ms, accumulated_active_ms, last_resume_at
             FROM time_blocks ORDER BY started_at",
        )?;
        let rows = stmt.query_map([], |r| {
            let kind: String = r.get(1)?;
            let status: String = r.get(7)?;
            Ok(TimeBlock {
                id: r.get(0)?,
                kind: BlockKind::parse(&kind).unwrap_or(BlockKind::Work),
                task_id: r.get(2)?,
                planned_ms: r.get(3)?,
                extension_ms: r.get(4)?,
                interruptions: r.get(5)?,
                actual_ms: r.get(6)?,
                status: BlockStatus::parse(&status).unwrap_or(BlockStatus::Cancelled),
                started_at: r.get(8)?,
                ended_at: r.get(9)?,
                end_at: r.get(10)?,
                paused_at: r.get(11)?,
                remaining_when_paused_ms: r.get(12)?,
                accumulated_active_ms: r.get(13)?,
                last_resume_at: r.get(14)?,
            })
        })?;
        for b in rows {
            blocks.push(b?);
        }
    }

    let (timer_state, current_block_id): (String, Option<String>) = conn
        .query_row(
            "SELECT timer_state, current_block_id FROM app_state WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or_else(|| (TimerState::Idle.as_str().to_string(), None));

    Ok(MachineState {
        timer_state: TimerState::parse(&timer_state).unwrap_or(TimerState::Idle),
        tasks,
        blocks,
        queue: queue.into_iter().map(|(_, id)| id).collect(),
        current_block_id,
    })
}
