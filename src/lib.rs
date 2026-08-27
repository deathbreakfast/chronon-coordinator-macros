//! Chronon×coordinator proc-macro crate (product hosts).
//!
//! Provides [`script`] for auto-registering scheduled Rust functions with upstream
//! `chronon-executor` inventory, product typed scheduling via `chronon-coordinator`,
//! and optional `default_job(...)` bootstrap.
//!
//! # Getting started
//!
//! ```ignore
//! use chronon_core::ScriptContext;
//! use chronon_valence_identity::valence_from_context;
//!
//! #[chronon_coordinator_macros::script(name = "daily_cleanup")]
//! pub async fn daily_cleanup(ctx: Box<dyn ScriptContext>) -> anyhow::Result<()> {
//!     let valence = valence_from_context(&*ctx)?;
//!     let _ = valence;
//!     Ok(())
//! }
//! ```
//!
//! # Feature flags
//!
//! | Feature | Default | Purpose |
//! |---------|---------|---------|
//! | `default-job` | yes | `default_job(...)` on [`script`] emits `DefaultJobDescriptor` inventory for boot bootstrap |

use proc_macro::TokenStream;

mod script;

#[cfg(feature = "default-job")]
mod script_default_job;

/// Marks an async function as a Chronon script.
///
/// # Requirements
///
/// - Function must be `async`
/// - First parameter must be `Box<dyn ScriptContext>`
/// - Return type must be `Result<()>` (for example `anyhow::Result<()>`)
/// - `name` attribute is required and must be unique
/// - optional `default_job(job = "...", cron = "..." | run_once | manual)`
/// - Parameters after `ScriptContext` must be simple identifiers
#[proc_macro_attribute]
pub fn script(attr: TokenStream, item: TokenStream) -> TokenStream {
    script::script_impl(attr, item)
}
