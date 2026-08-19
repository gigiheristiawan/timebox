//! Pure queue operations. The queue is the product — a playlist for work — so
//! every mutation is a named operation rather than an ad-hoc splice.

use super::model::TaskId;

/// Move a task to the front. Used when a task starts.
pub fn bring_to_front(queue: &mut Vec<TaskId>, id: &TaskId) {
    queue.retain(|t| t != id);
    queue.insert(0, id.clone());
}

/// Move a task to the back. Every "I am leaving this task" path routes here —
/// pending, skip, and switch alike — so leaving always costs a lap (SPEC D2/D10).
pub fn rotate_to_back(queue: &mut Vec<TaskId>, id: &TaskId) {
    queue.retain(|t| t != id);
    queue.push(id.clone());
}

/// Remove a task entirely. Used when it is completed or cancelled.
pub fn remove(queue: &mut Vec<TaskId>, id: &TaskId) {
    queue.retain(|t| t != id);
}

/// Reorder by drag-and-drop: place `moved` immediately before `before`.
pub fn move_before(queue: &mut Vec<TaskId>, moved: &TaskId, before: &TaskId) {
    if moved == before || !queue.contains(moved) {
        return;
    }
    queue.retain(|t| t != moved);
    match queue.iter().position(|t| t == before) {
        Some(i) => queue.insert(i, moved.clone()),
        None => queue.push(moved.clone()),
    }
}

pub fn head(queue: &[TaskId]) -> Option<&TaskId> {
    queue.first()
}

/// The first task that is not the one currently running.
pub fn next_after<'a>(queue: &'a [TaskId], current: Option<&TaskId>) -> Option<&'a TaskId> {
    queue.iter().find(|t| Some(*t) != current)
}
