use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    path::PathBuf,
};
use steel_core::{bootstrap::init_globals, command::CommandRegistry, extension::ExtensionHooks};
use steel_extension_api::{
    hooks::{ServerTickAction, apply_filters, do_action},
    host,
};
use steel_hook_example_contract::{Score, Scored};

#[test]
fn separate_extensions_reorder_the_filter_chain_across_the_native_boundary() -> Result<(), String> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug");
    let mut hooks = ExtensionHooks::default();
    let mut extensions = Vec::new();
    // Deliberately load base first: its add-five handler must still run after bonus.
    for name in ["steel_hook_base_extension", "steel_hook_bonus_extension"] {
        let path = directory.join(format!("{DLL_PREFIX}{name}{DLL_SUFFIX}"));
        // SAFETY: these separately built trusted examples supply the checked v2
        // descriptors; the loader pins their libraries before installing callbacks.
        extensions.push(unsafe { host::load(&path, "", &mut hooks) }?);
    }
    init_globals();
    let _commands = CommandRegistry::new().freeze()?;
    host::initialize_all(extensions, &mut hooks)?;
    let mut score = Score { value: 10 };
    apply_filters(&mut score).match_owned(Ok, |e| Err(e.to_string()))?;
    assert_eq!(
        score.value, 25,
        "bonus doubles before the base adds five; registration order would produce 30"
    );
    do_action(&Scored { value: score.value }).match_owned(Ok, |e| Err(e.to_string()))?;
    do_action(&ServerTickAction { tick_count: 42 }).match_owned(Ok, |e| Err(e.to_string()))?;
    Ok(())
}
