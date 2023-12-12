use crate::apis::coredb_types::{Backup, CoreDBSpec};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(components(schemas(Backup, CoreDBSpec)))]
pub struct ApiDocv1;
