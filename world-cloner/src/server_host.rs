//! [`HostWorlds`] for the real host [`Server`].

use steel_extension_api::{Operation, OperationState, PlayerId, Server, WorldSettings};
use world_cloner_api::{CloneSettings, CloneTarget, TemplateStatus};

use crate::host::{HostWorlds, OpStatus};

/// One tick's server handle; the host never lets it outlive the callback it was given to.
pub struct ServerHost<'a>(pub Server<'a>);

/// Converts an ABI result into a `Result` with an owned error message.
macro_rules! owned {
    ($result:expr $(,)?) => {
        $result.match_owned(Ok, |error| Err(error.to_string()))
    };
}

impl HostWorlds for ServerHost<'_> {
    type Operation = Operation;
    type Player = PlayerId;

    fn template_status(&self, key: &str) -> Option<TemplateStatus> {
        self.0.world_template_status(key.into()).into()
    }

    fn load_template(&self, key: &str, path: &str) -> Result<Operation, String> {
        owned!(self.0.load_world_template(key.into(), path.into()))
    }

    fn clone_world(
        &self,
        template_key: &str,
        key: &str,
        target: CloneTarget,
        settings: CloneSettings,
    ) -> Result<Operation, String> {
        owned!(
            self.0
                .clone_world(template_key.into(), key.into(), target, settings),
        )
    }

    fn create_world(
        &self,
        key: &str,
        generator: &str,
        target: CloneTarget,
        settings: WorldSettings,
    ) -> Result<Operation, String> {
        owned!(
            self.0
                .create_world(key.into(), generator.into(), target, settings),
        )
    }

    fn remove_world(&self, key: &str) -> Result<Operation, String> {
        owned!(self.0.remove_world(key.into()))
    }

    fn operation_status(&self, operation: Operation) -> Result<OpStatus, String> {
        let status = owned!(self.0.operation_status(operation))?;
        let value = status.value.to_string();
        Ok(match status.state {
            OperationState::Pending => OpStatus::Pending,
            OperationState::Ready => OpStatus::Ready(value),
            OperationState::Failed => OpStatus::Failed(value),
        })
    }

    fn message(&self, player: PlayerId, text: &str) {
        // The player may have left; a missed progress message is not an error.
        let _ = self.0.message(player, text.into());
    }
}
