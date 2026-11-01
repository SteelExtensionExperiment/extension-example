//! Supplies an earlier score filter through the same shared contract as the base extension.

use steel_extension_api::{ApiResult, ExtensionContext, Hooks, stabby};
use steel_hook_example_contract::Score;

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    hooks
        .add_filter::<Score>(double)
        .before("hook-base-example")
        .register()
}

#[stabby::export(callback)]
fn double(score: &mut Score) {
    score.value = score.value.saturating_mul(2);
}

// The score transformation is fully installed during pre-init.
fn init(_context: ExtensionContext<'_>) -> ApiResult<()> {
    Ok(()).into()
}

steel_extension_api::export_extension!("hook-bonus-example", pre_init, init);
