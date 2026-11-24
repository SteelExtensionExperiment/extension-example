//! The `world-cloner:clone/v1` service implementation.

use steel_extension_api::{
    ApiResult, Operation, Server,
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};
use world_cloner_api::{CloneRequest, CreateRequest, TemplateInfo, WorldCloner};

use crate::runtime::Runtime;
use crate::server_host::ServerHost;

/// Serves the contract from the shared runtime state.
pub struct ServiceImpl(pub &'static Runtime);

fn into_api(result: Result<Operation, String>) -> ApiResult<Operation> {
    result.map_err(ApiString::from).into()
}

#[stabby::export(callback)]
impl WorldCloner for ServiceImpl {
    fn templates(&self, server: Server<'_>) -> ApiVec<TemplateInfo> {
        self.0
            .cloner
            .templates(&ServerHost(server))
            .into_iter()
            .collect()
    }

    fn clone_world(&self, server: Server<'_>, request: CloneRequest) -> ApiResult<Operation> {
        into_api(self.0.cloner.clone_world(&ServerHost(server), request))
    }

    fn create_world(&self, server: Server<'_>, request: CreateRequest) -> ApiResult<Operation> {
        into_api(self.0.cloner.create_world(&ServerHost(server), request))
    }

    fn remove_world(&self, server: Server<'_>, key: Str<'_>) -> ApiResult<Operation> {
        into_api(
            self.0
                .cloner
                .remove_world(&ServerHost(server), key.as_str()),
        )
    }
}
