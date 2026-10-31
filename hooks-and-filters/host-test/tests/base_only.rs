use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    path::PathBuf,
};
use steel_core::{bootstrap::init_globals, command::CommandRegistry, extension::ExtensionHooks};
use steel_extension_api::{hooks::apply_filters, host};
use steel_hook_example_contract::Score;

#[test]
fn the_base_example_works_when_the_ordering_target_is_absent() -> Result<(), String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/debug")
        .join(format!("{DLL_PREFIX}steel_hook_base_extension{DLL_SUFFIX}"));
    let mut hooks = ExtensionHooks::default();
    // SAFETY: the independently compiled trusted base example has the checked v2
    // descriptor; load pins its code before retaining its callbacks.
    let extension = unsafe { host::load(&path, "", &mut hooks) }?;
    init_globals();
    let _commands = CommandRegistry::new().freeze()?;
    host::initialize_all(vec![extension], &mut hooks)?;
    let mut score = Score { value: 10 };
    apply_filters(&mut score).match_owned(Ok, |e| Err(e.to_string()))?;
    assert_eq!(score.value, 15);
    Ok(())
}
