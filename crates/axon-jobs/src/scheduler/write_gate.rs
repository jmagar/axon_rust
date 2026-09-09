//! Compatibility names for the shared SQLite writer admission primitive.

/// Process-local admission gate for scheduler mutations sharing one SQLite DB.
///
/// SQLite admits one writer at a time. Without this gate, concurrent provider
/// calls can park every SQLx connection worker inside SQLite's busy handler,
/// starving unrelated job heartbeats and control-plane reads of a pool slot.
///
/// The gate is intentionally process-local even though the DB is shared with
/// short-lived CLI processes: cross-process writers are serialized by SQLite's
/// own write lock, so the accepted bound is that a gate holder may stall up to
/// the busy timeout behind an external writer while in-process writers queue
/// behind the gate.
pub use axon_core::sqlite::{SqliteWriteGate, SqliteWriteGuard};

/// Backward-compatible scheduler-facing name for the shared SQLite writer gate.
pub type SchedulerWriteGate = SqliteWriteGate;
