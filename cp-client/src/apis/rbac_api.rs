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
use serde::{Deserialize, Serialize};
use crate::{apis::ResponseContent, models};
use super::{Error, configuration};


/// struct for typed errors of method [`delete_policy`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeletePolicyError {
    Status401(models::ErrorResponseSchema),
    Status403(models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_actions`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetActionsError {
    Status401(models::ErrorResponseSchema),
    Status403(models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_policies`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetPoliciesError {
    Status401(models::ErrorResponseSchema),
    Status403(models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`get_roles`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GetRolesError {
    Status401(models::ErrorResponseSchema),
    Status403(models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}

/// struct for typed errors of method [`set_policy`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SetPolicyError {
    Status400(models::ErrorResponseSchema),
    Status401(models::ErrorResponseSchema),
    Status403(models::ErrorResponseSchema),
    UnknownValue(serde_json::Value),
}


/// Delete a policy
pub async fn delete_policy(configuration: &configuration::Configuration, org_id: &str, policy_input: models::PolicyInput) -> Result<String, Error<DeletePolicyError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_org_id = org_id;
    let p_policy_input = policy_input;

    let uri_str = format!("{}/api/v1/orgs/{org_id}/policies", configuration.base_path, org_id=crate::apis::urlencode(p_org_id));
    let mut req_builder = configuration.client.request(reqwest::Method::DELETE, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    if let Some(ref token) = configuration.bearer_access_token {
        req_builder = req_builder.bearer_auth(token.to_owned());
    };
    req_builder = req_builder.json(&p_policy_input);

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<DeletePolicyError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent { status, content, entity }))
    }
}

/// Get all actions
pub async fn get_actions(configuration: &configuration::Configuration, org_id: &str) -> Result<Vec<models::Action>, Error<GetActionsError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_org_id = org_id;

    let uri_str = format!("{}/api/v1/orgs/{org_id}/actions", configuration.base_path, org_id=crate::apis::urlencode(p_org_id));
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    if let Some(ref token) = configuration.bearer_access_token {
        req_builder = req_builder.bearer_auth(token.to_owned());
    };

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetActionsError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent { status, content, entity }))
    }
}

/// Get all policies
pub async fn get_policies(configuration: &configuration::Configuration, org_id: &str) -> Result<Vec<models::PolicyData>, Error<GetPoliciesError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_org_id = org_id;

    let uri_str = format!("{}/api/v1/orgs/{org_id}/policies", configuration.base_path, org_id=crate::apis::urlencode(p_org_id));
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    if let Some(ref token) = configuration.bearer_access_token {
        req_builder = req_builder.bearer_auth(token.to_owned());
    };

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetPoliciesError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent { status, content, entity }))
    }
}

/// Get all roles
pub async fn get_roles(configuration: &configuration::Configuration, org_id: &str) -> Result<Vec<models::Role>, Error<GetRolesError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_org_id = org_id;

    let uri_str = format!("{}/api/v1/orgs/{org_id}/roles", configuration.base_path, org_id=crate::apis::urlencode(p_org_id));
    let mut req_builder = configuration.client.request(reqwest::Method::GET, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    if let Some(ref token) = configuration.bearer_access_token {
        req_builder = req_builder.bearer_auth(token.to_owned());
    };

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<GetRolesError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent { status, content, entity }))
    }
}

/// Create or update a policy
pub async fn set_policy(configuration: &configuration::Configuration, org_id: &str, policy_input: models::PolicyInput) -> Result<String, Error<SetPolicyError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_org_id = org_id;
    let p_policy_input = policy_input;

    let uri_str = format!("{}/api/v1/orgs/{org_id}/policies", configuration.base_path, org_id=crate::apis::urlencode(p_org_id));
    let mut req_builder = configuration.client.request(reqwest::Method::POST, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    if let Some(ref token) = configuration.bearer_access_token {
        req_builder = req_builder.bearer_auth(token.to_owned());
    };
    req_builder = req_builder.json(&p_policy_input);

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        serde_json::from_str(&content).map_err(Error::from)
    } else {
        let content = resp.text().await?;
        let entity: Option<SetPolicyError> = serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent { status, content, entity }))
    }
}

