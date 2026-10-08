//! The only destructive entrypoints in Wrasse: `remove` and `delegate` (PLAN.md §4.1).
//! Implemented in Phase 2. Nothing outside `safety/`, `quarantine/` and `platform/`
//! may delete, rename or spawn processes; CI enforces this.
