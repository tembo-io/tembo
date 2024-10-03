/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

use reqwest;

use super::{configuration, Error};
use crate::apis::ResponseContent;
use crate::models::InstanceEvent;

/// struct for typed errors of method [`create_instance`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateInstanceError {
    Status400(crate::models::ErrorResponseSchema),
    Status401(String),
    Status403(crate::models::ErrorResponseSchema),
    Status409(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`delete_instance`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeleteInstanceError {
    Status400(crate::models::ErrorResponseSchema),
    Status401(crate::models::ErrorResponseSchema),
    Status403(crate::models::ErrorResponseSchema),
    Status409(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_all`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetAllError {
    Status400(crate::models::ErrorResponseSchema),
    Status401(crate::models::ErrorResponseSchema),
    Status403(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_instance`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetInstanceError {
    Status400(crate::models::ErrorResponseSchema),
    Status401(crate::models::ErrorResponseSchema),
    Status403(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_schema`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetSchemaError {
    Status401(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`instance_event`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InstanceEventError {
    Status401(crate::models::ErrorResponseSchema),
    Status403(crate::models::ErrorResponseSchema),
    Status409(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`patch_instance`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatchInstanceError {
    Status400(crate::models::ErrorResponseSchema),
    Status401(String),
    Status403(crate::models::ErrorResponseSchema),
    Status409(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`put_instance`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PutInstanceError {
    Status400(crate::models::ErrorResponseSchema),
    Status401(crate::models::ErrorResponseSchema),
    Status403(crate::models::ErrorResponseSchema),
    Status409(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`restore_instance`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RestoreInstanceError {
    Status400(crate::models::ErrorResponseSchema),
    Status401(String),
    Status403(crate::models::ErrorResponseSchema),
    Status409(crate::models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// Create a new Tembo instance
pub async fn create_instance(
    configuration: &configuration::Configuration,
    org_id: &str,
    create_instance: crate::models::CreateInstance,
) -> Result<crate::models::Instance, Error<()>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{}/instances",
        local_var_configuration.base_path,
        crate::apis::urlencode(org_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::POST, &local_var_uri_str);

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder = local_var_req_builder
            .header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder =
            local_var_req_builder.bearer_auth(local_var_token.to_owned());
    }
    local_var_req_builder = local_var_req_builder.json(&create_instance);

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if local_var_status.is_success() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: None,
        };
        Err(Error::ResponseError(local_var_error))
    }
}


/// Delete an existing Tembo instance
pub async fn delete_instance(
    configuration: &configuration::Configuration,
    org_id: &str,
    instance_id: &str,
) -> Result<crate::models::Instance, Error<DeleteInstanceError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{org_id}/instances/{instance_id}",
        local_var_configuration.base_path,
        org_id = crate::apis::urlencode(org_id),
        instance_id = crate::apis::urlencode(instance_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::DELETE, local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<DeleteInstanceError> =
            serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}

/// Get all Tembo instances in an organization
pub async fn get_all(
    configuration: &configuration::Configuration,
    org_id: &str,
) -> Result<Vec<crate::models::Instance>, Error<GetAllError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{org_id}/instances",
        local_var_configuration.base_path,
        org_id = crate::apis::urlencode(org_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::GET, local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<GetAllError> = serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}

/// Get an existing Tembo instance
pub async fn get_instance(
    configuration: &configuration::Configuration,
    org_id: &str,
    instance_id: &str,
) -> Result<crate::models::Instance, Error<GetInstanceError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{org_id}/instances/{instance_id}",
        local_var_configuration.base_path,
        org_id = crate::apis::urlencode(org_id),
        instance_id = crate::apis::urlencode(instance_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::GET, local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<GetInstanceError> =
            serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}

/// Get the json-schema for an instance
pub async fn get_schema(
    configuration: &configuration::Configuration,
) -> Result<crate::models::ErrorResponseSchema, Error<GetSchemaError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/instances/schema",
        local_var_configuration.base_path
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::GET, local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<GetSchemaError> =
            serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}

/// Lifecycle events for a Tembo instance  Currently only supports restart
pub async fn instance_event(
    configuration: &configuration::Configuration,
    org_id: &str,
    event_type: InstanceEvent,
    instance_id: &str,
) -> Result<crate::models::Instance, Error<InstanceEventError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{org_id}/instances/{instance_id}",
        local_var_configuration.base_path,
        org_id = crate::apis::urlencode(org_id),
        instance_id = crate::apis::urlencode(instance_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::POST, local_var_uri_str.as_str());

    local_var_req_builder = local_var_req_builder.query(&[("event_type", &event_type.to_string())]);
    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<InstanceEventError> =
            serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}

/// Update attributes on an existing Tembo instance
pub async fn patch_instance(
    configuration: &configuration::Configuration,
    org_id: &str,
    instance_id: &str,
    patch_instance: crate::models::PatchInstance,
) -> Result<crate::models::Instance, Error<PatchInstanceError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{org_id}/instances/{instance_id}",
        local_var_configuration.base_path,
        org_id = crate::apis::urlencode(org_id),
        instance_id = crate::apis::urlencode(instance_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::PATCH, local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };
    local_var_req_builder = local_var_req_builder.json(&patch_instance);

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<PatchInstanceError> =
            serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}

/// Replace all attributes of an existing Tembo instance
pub async fn put_instance(
    configuration: &configuration::Configuration,
    org_id: &str,
    instance_id: &str,
    update_instance: crate::models::UpdateInstance,
) -> Result<crate::models::Instance, Error<PutInstanceError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{org_id}/instances/{instance_id}",
        local_var_configuration.base_path,
        org_id = crate::apis::urlencode(org_id),
        instance_id = crate::apis::urlencode(instance_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::PUT, local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };
    local_var_req_builder = local_var_req_builder.json(&update_instance);

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<PutInstanceError> =
            serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}

/// Restore a Tembo instance
pub async fn restore_instance(
    configuration: &configuration::Configuration,
    org_id: &str,
    restore_instance: crate::models::RestoreInstance,
) -> Result<crate::models::Instance, Error<RestoreInstanceError>> {
    let local_var_configuration = configuration;

    let local_var_client = &local_var_configuration.client;

    let local_var_uri_str = format!(
        "{}/api/v1/orgs/{org_id}/restore",
        local_var_configuration.base_path,
        org_id = crate::apis::urlencode(org_id)
    );
    let mut local_var_req_builder =
        local_var_client.request(reqwest::Method::POST, local_var_uri_str.as_str());

    if let Some(ref local_var_user_agent) = local_var_configuration.user_agent {
        local_var_req_builder =
            local_var_req_builder.header(reqwest::header::USER_AGENT, local_var_user_agent.clone());
    }
    if let Some(ref local_var_token) = local_var_configuration.bearer_access_token {
        local_var_req_builder = local_var_req_builder.bearer_auth(local_var_token.to_owned());
    };
    local_var_req_builder = local_var_req_builder.json(&restore_instance);

    let local_var_req = local_var_req_builder.build()?;
    let local_var_resp = local_var_client.execute(local_var_req).await?;

    let local_var_status = local_var_resp.status();
    let local_var_content = local_var_resp.text().await?;

    if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
        serde_json::from_str(&local_var_content).map_err(Error::from)
    } else {
        let local_var_entity: Option<RestoreInstanceError> =
            serde_json::from_str(&local_var_content).ok();
        let local_var_error = ResponseContent {
            status: local_var_status,
            content: local_var_content,
            entity: local_var_entity,
        };
        Err(Error::ResponseError(local_var_error))
    }
}
