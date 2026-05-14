//! Run services — split by concern.
//!
//! - [`submission`]: write paths (`create_run` with its validation
//!   pipeline, `delete_run`) plus the [`submission::CreateRunRequest`]
//!   input DTO.
//! - [`read`]: read paths (`get_run`, `list_runs`, `get_run_defaults`)
//!   plus the [`read::RunDetail`] / [`read::RunDefaults`] /
//!   [`read::RunFilters`] DTOs.
//!
//! All public items are re-exported here so external callers continue to use
//! `crate::services::runs::<name>`.

mod read;
mod submission;

pub use read::*;
pub use submission::*;
