//! Integration contract: expanded `#[script]` surface hosts actually call.

#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::unused_async
)]

use chronon_core::ScriptContext;

#[chronon_coordinator_macros::script(name = "contract_tick")]
pub async fn contract_tick(ctx: Box<dyn ScriptContext>) -> anyhow::Result<()> {
    let _ = ctx;
    Ok(())
}

#[chronon_coordinator_macros::script(
    name = "contract_sweep",
    default_job(job = "contract-sweep-job", cron = "*/15 * * * * *")
)]
pub async fn contract_sweep(ctx: Box<dyn ScriptContext>) -> anyhow::Result<()> {
    let _ = ctx;
    Ok(())
}

#[test]
fn script_macro_handle_and_name_happy() {
    assert_eq!(ContractTickScript::NAME, "contract_tick");
    let handle = ContractTickScript::handle();
    assert_eq!(handle.name(), "contract_tick");
}

#[test]
fn script_macro_default_job_expands_happy() {
    assert_eq!(ContractSweepScript::NAME, "contract_sweep");
    let handle = ContractSweepScript::handle();
    assert_eq!(handle.name(), "contract_sweep");
}
