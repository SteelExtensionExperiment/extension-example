//! The world cloner service logic, written against [`HostWorlds`].
//!
//! The extension holds no template data; the host registry is the source of truth (FR-004).

use std::path::PathBuf;

use steel_extension_api::{
    BlockPos, Difficulty as HostDifficulty, GameMode as HostGameMode, WorldSettings,
};
use world_cloner_api::{CloneRequest, CloneTarget, CreateRequest, TemplateInfo, TemplateState};

use crate::config::{Config, CreateConfig, Difficulty, GameMode, TemplateConfig};
use crate::host::HostWorlds;

const HOST_KEY_DOMAIN: &str = "world-cloner";

impl From<GameMode> for HostGameMode {
    fn from(mode: GameMode) -> Self {
        match mode {
            GameMode::Survival => Self::Survival,
            GameMode::Creative => Self::Creative,
            GameMode::Adventure => Self::Adventure,
            GameMode::Spectator => Self::Spectator,
        }
    }
}

impl From<Difficulty> for HostDifficulty {
    fn from(difficulty: Difficulty) -> Self {
        match difficulty {
            Difficulty::Peaceful => Self::Peaceful,
            Difficulty::Easy => Self::Easy,
            Difficulty::Normal => Self::Normal,
            Difficulty::Hard => Self::Hard,
        }
    }
}

impl From<CreateConfig> for WorldSettings {
    fn from(config: CreateConfig) -> Self {
        let [x, y, z] = config.spawn;
        Self {
            seed: config.seed,
            spawn: BlockPos { x, y, z },
            default_game_mode: config.default_game_mode.into(),
            difficulty: config.difficulty.into(),
        }
    }
}

/// Configured templates plus the data directory they are resolved against.
#[derive(Debug, Clone)]
pub struct Cloner {
    config: Config,
    data_dir: PathBuf,
}

impl Cloner {
    #[must_use]
    pub const fn new(config: Config, data_dir: PathBuf) -> Self {
        Self { config, data_dir }
    }

    /// The host key of a template. The only place that builds it.
    #[must_use]
    pub fn host_key(name: &str) -> String {
        format!("{HOST_KEY_DOMAIN}:{name}")
    }

    #[must_use]
    pub fn template_configs(&self) -> &[TemplateConfig] {
        &self.config.templates
    }

    /// The absolute folder of a template, as the host expects it.
    ///
    /// # Errors
    /// The path is not valid UTF-8.
    pub fn template_path(&self, template: &TemplateConfig) -> Result<String, String> {
        let path = self.data_dir.join(&template.path);
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("path {} is not valid UTF-8", path.display()))
    }

    /// Every configured template with its host state; one the host does not know yet is Loading.
    #[must_use]
    pub fn templates<H: HostWorlds>(&self, host: &H) -> Vec<TemplateInfo> {
        self.config
            .templates
            .iter()
            .map(|template| {
                let (state, message) = host
                    .template_status(&Self::host_key(&template.name))
                    .map_or((TemplateState::Loading, String::new()), |status| {
                        (status.state, status.message.to_string())
                    });
                TemplateInfo {
                    name: template.name.as_str().into(),
                    state,
                    message: message.as_str().into(),
                }
            })
            .collect()
    }

    /// Starts cloning a Ready template; an unset game mode takes the template's configured one.
    ///
    /// # Errors
    /// The template is unknown, still loading or failed, or the host refuses the request.
    pub fn clone_world<H: HostWorlds>(
        &self,
        host: &H,
        request: CloneRequest,
    ) -> Result<H::Operation, String> {
        let name = request.template.as_str();
        let template = self
            .config
            .templates
            .iter()
            .find(|template| template.name == name)
            .ok_or_else(|| format!("unknown template {name}"))?;
        let template_key = Self::host_key(name);
        match host.template_status(&template_key) {
            Some(status) if status.state == TemplateState::Ready => {}
            Some(status) if status.state == TemplateState::Failed => {
                return Err(status.message.to_string());
            }
            _ => return Err("template is still loading".to_owned()),
        }
        let mut settings = request.settings;
        if settings.default_game_mode.is_none() {
            settings.default_game_mode =
                Some(HostGameMode::from(template.default_game_mode)).into();
        }
        host.clone_world(&template_key, &request.key, request.target, settings)
    }

    /// A create request for the admin command: the `[create]` settings of the configuration.
    #[must_use]
    pub fn create_request(&self, generator: &str, key: &str, target: CloneTarget) -> CreateRequest {
        CreateRequest {
            key: key.into(),
            generator: generator.into(),
            target,
            settings: self.config.create.into(),
        }
    }

    /// Starts creating a generated world with the request's complete settings.
    ///
    /// # Errors
    /// The host refuses the request.
    pub fn create_world<H: HostWorlds>(
        &self,
        host: &H,
        request: CreateRequest,
    ) -> Result<H::Operation, String> {
        let settings: WorldSettings = request.settings;
        host.create_world(&request.key, &request.generator, request.target, settings)
    }

    /// Starts removing a runtime world.
    ///
    /// # Errors
    /// The host refuses the request.
    pub fn remove_world<H: HostWorlds>(&self, host: &H, key: &str) -> Result<H::Operation, String> {
        host.remove_world(key)
    }
}

#[cfg(test)]
mod tests {
    use steel_extension_api::{BlockPos, Difficulty};
    use world_cloner_api::{CloneSettings, CloneTarget, TemplateStatus};

    use super::*;
    use crate::host::fake::{Call, FakeHost};

    fn cloner() -> Cloner {
        let toml = "[[templates]]\nname = \"ready\"\npath = \"t/ready\"\n\
                    default_game_mode = \"adventure\"\n\
                    [[templates]]\nname = \"loading\"\npath = \"t/loading\"\n\
                    [[templates]]\nname = \"broken\"\npath = \"t/broken\"\n\
                    [[templates]]\nname = \"unknown_to_host\"\npath = \"t/u\"\n";
        Cloner::new(Config::parse(toml).unwrap(), PathBuf::from("/data"))
    }

    fn status(state: TemplateState, message: &str) -> TemplateStatus {
        TemplateStatus {
            state,
            message: message.into(),
            chunks: 0,
        }
    }

    fn host() -> FakeHost {
        let host = FakeHost::default();
        let mut statuses = host.statuses.borrow_mut();
        statuses.insert(
            "world-cloner:ready".to_owned(),
            status(TemplateState::Ready, ""),
        );
        statuses.insert(
            "world-cloner:loading".to_owned(),
            status(TemplateState::Loading, ""),
        );
        statuses.insert(
            "world-cloner:broken".to_owned(),
            status(TemplateState::Failed, "region folder missing"),
        );
        drop(statuses);
        host
    }

    fn no_overrides() -> CloneSettings {
        CloneSettings {
            seed: None.into(),
            spawn: None.into(),
            default_game_mode: None.into(),
            difficulty: None.into(),
        }
    }

    fn request(template: &str, settings: CloneSettings) -> CloneRequest {
        CloneRequest {
            template: template.into(),
            key: "arena:one".into(),
            target: CloneTarget::NewTemporaryDomain,
            settings,
        }
    }

    #[test]
    fn clone_failures_never_reach_the_host() {
        let host = host();
        let cloner = cloner();
        let clone = |name| {
            cloner
                .clone_world(&host, request(name, no_overrides()))
                .unwrap_err()
        };
        assert_eq!(clone("nope"), "unknown template nope");
        assert_eq!(clone("loading"), "template is still loading");
        assert_eq!(clone("unknown_to_host"), "template is still loading");
        assert_eq!(clone("broken"), "region folder missing");
        assert!(host.calls.borrow().is_empty());
    }

    #[test]
    fn clone_uses_the_host_key_and_fills_the_default_game_mode() {
        let host = host();
        cloner()
            .clone_world(&host, request("ready", no_overrides()))
            .unwrap();
        let calls = host.calls.borrow();
        let [
            Call::Clone {
                template_key,
                key,
                target,
                settings,
            },
        ] = calls.as_slice()
        else {
            panic!("expected one clone call, got {calls:?}");
        };
        assert_eq!(template_key, "world-cloner:ready");
        assert_eq!(key, "arena:one");
        assert_eq!(*target, CloneTarget::NewTemporaryDomain);
        assert!(matches!(
            settings.default_game_mode.as_ref(),
            Some(HostGameMode::Adventure)
        ));
        assert!(
            settings.seed.is_none() && settings.spawn.is_none() && settings.difficulty.is_none()
        );
    }

    #[test]
    fn explicit_game_mode_and_other_overrides_pass_through() {
        let host = host();
        let settings = CloneSettings {
            seed: Some(7).into(),
            spawn: Some(BlockPos { x: 1, y: 2, z: 3 }).into(),
            default_game_mode: Some(HostGameMode::Creative).into(),
            difficulty: Some(Difficulty::Hard).into(),
        };
        cloner()
            .clone_world(&host, request("ready", settings))
            .unwrap();
        let calls = host.calls.borrow();
        let [Call::Clone { settings, .. }] = calls.as_slice() else {
            panic!("expected one clone call");
        };
        assert!(matches!(
            settings.default_game_mode.as_ref(),
            Some(HostGameMode::Creative)
        ));
        assert_eq!(settings.seed.as_ref(), Some(&7));
        assert_eq!(
            settings.spawn.as_ref(),
            Some(&BlockPos { x: 1, y: 2, z: 3 })
        );
        assert!(matches!(
            settings.difficulty.as_ref(),
            Some(Difficulty::Hard)
        ));
    }

    #[test]
    fn host_refusal_is_passed_on_unchanged() {
        let host = host();
        host.refused
            .borrow_mut()
            .insert("arena:one".to_owned(), "key in use".to_owned());
        let result = cloner().clone_world(&host, request("ready", no_overrides()));
        assert_eq!(result, Err("key in use".to_owned()));
    }

    #[test]
    fn create_and_remove_pass_through() {
        let host = host();
        let settings = WorldSettings {
            seed: 42,
            spawn: BlockPos { x: 0, y: 64, z: 0 },
            default_game_mode: HostGameMode::Creative,
            difficulty: Difficulty::Easy,
        };
        let create = CreateRequest {
            key: "arena:gen".into(),
            generator: "example:strange".into(),
            target: CloneTarget::ExistingDomain,
            settings,
        };
        cloner().create_world(&host, create).unwrap();
        cloner().remove_world(&host, "arena:gen").unwrap();
        let calls = host.calls.borrow();
        let [
            Call::Create {
                key,
                generator,
                target,
                settings,
            },
            Call::Remove { key: removed },
        ] = calls.as_slice()
        else {
            panic!("expected create then remove, got {calls:?}");
        };
        assert_eq!(
            (key.as_str(), generator.as_str()),
            ("arena:gen", "example:strange")
        );
        assert_eq!(*target, CloneTarget::ExistingDomain);
        assert_eq!((settings.seed, settings.spawn.y), (42, 64));
        assert!(matches!(settings.default_game_mode, HostGameMode::Creative));
        assert!(matches!(settings.difficulty, Difficulty::Easy));
        assert_eq!(removed, "arena:gen");
    }

    #[test]
    fn templates_report_host_state_and_unknown_as_loading() {
        let listed = cloner().templates(&host());
        let rows: Vec<_> = listed
            .iter()
            .map(|info| (info.name.as_str(), info.state, info.message.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("ready", TemplateState::Ready, ""),
                ("loading", TemplateState::Loading, ""),
                ("broken", TemplateState::Failed, "region folder missing"),
                ("unknown_to_host", TemplateState::Loading, ""),
            ]
        );
    }

    #[test]
    fn template_path_joins_the_data_directory() {
        let cloner = cloner();
        let path = cloner.template_path(&cloner.template_configs()[0]).unwrap();
        assert_eq!(path, "/data/t/ready");
    }
}
