//! The domain core. Pure: no I/O, no Tauri, no clock of its own.
//!
//! Every rule that defines what TimeBox *is* lives here and nowhere else
//! (SPEC R6/R7). The TypeScript side formats; it never decides.

pub mod menubar;
pub mod model;
pub mod queue;
pub mod timer_machine;

#[cfg(test)]
mod tests;
