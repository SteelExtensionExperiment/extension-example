//! Process-wide extension state and the command and tick entry points that use it.
//!
//! Lifecycle: `pre_init` registers the callbacks, `init` sets the state. The host builds command
//! trees before `init` but only executes commands and ticks after every extension is initialized
//! (`steel/src/lib.rs`: `initialize_all`, then `Server::new_with_extensions`), and a failed `init`
//! stops startup. The callbacks still read the state through [`runtime`] and do nothing while it
//! is unset.

use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use steel_extension_api::command::Invocation;
use steel_extension_api::{Operation, PlayerId, Server, log};

use crate::commands::{self, Reporter, Request};
use crate::server_host::ServerHost;
use crate::service::Cloner;
use crate::startup::{Startup, StartupEvent};

/// Command result for a command that ran.
const COMMAND_SUCCESS: i32 = 1;

/// Everything the callbacks share.
pub struct Runtime {
    pub cloner: Cloner,
    startup: Mutex<Startup<Operation>>,
    reporter: Mutex<Reporter<Operation, PlayerId>>,
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// The state set by `init`, or `None` before it.
pub fn runtime() -> Option<&'static Runtime> {
    RUNTIME.get()
}

/// Stores the state `init` built and returns it. `init` runs once per load.
pub fn install(cloner: Cloner) -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime {
        cloner,
        startup: Mutex::default(),
        reporter: Mutex::default(),
    })
}

/// Locks a mutex whose holders never leave the data half-updated.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs a parsed `/worldcloner` request and sends the immediate feedback to the sender.
///
/// # Errors
/// The extension is not initialized, or the request is refused.
pub fn handle(ctx: &Invocation<'_, '_>, request: Request) -> Result<i32, String> {
    let runtime = runtime().ok_or("world-cloner is not initialized")?;
    let host = ServerHost(ctx.server());
    let feedback = commands::execute(
        &host,
        &runtime.cloner,
        &mut lock(&runtime.reporter),
        request,
        ctx.player().ok(),
    )?;
    ctx.send(&feedback);
    Ok(COMMAND_SUCCESS)
}

/// Template names for command completion.
pub fn template_names() -> Vec<String> {
    runtime().map_or_else(Vec::new, |runtime| {
        runtime
            .cloner
            .template_configs()
            .iter()
            .map(|template| template.name.clone())
            .collect()
    })
}

/// Server tick: starts and polls template loads, and reports finished command operations. Logs
/// after the locks are released.
pub extern "C" fn on_tick(server: Server<'_>, _tick_count: u64) {
    let Some(runtime) = runtime() else {
        return;
    };
    let host = ServerHost(server);
    let events = lock(&runtime.startup).tick(&host, &runtime.cloner);
    let lines = lock(&runtime.reporter).poll(&host);
    for event in events {
        match event {
            StartupEvent::Ready { .. } => log::info(event.to_string()),
            StartupEvent::Failed { .. } => log::warn(event.to_string()),
        }
    }
    for line in lines {
        log::info(line);
    }
}
