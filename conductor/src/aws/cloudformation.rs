//use crate::errors::ConductorError;
use aws_config::SdkConfig;
use aws_sdk_cloudformation::{
    config::Region,
    types::{Capability, Parameter, StackStatus},
    Client,
};
use log::{error, info};
use std::sync::Arc;

use crate::errors::ConductorError;

#[derive(Clone)]
pub struct CloudFormationParams {
    pub bucket_name: String,
    pub read_path_prefix: String,
    pub write_path_prefix: String,
    pub role_name: String,
    pub namespace: String,
    pub service_account_name: String,
}

impl CloudFormationParams {
    fn parameters(self) -> Vec<Parameter> {
        vec![
            Parameter::builder()
                .parameter_key("BucketName")
                .parameter_value(self.bucket_name)
                .build(),
            Parameter::builder()
                .parameter_key("ReadPathPrefix")
                .parameter_value(self.read_path_prefix)
                .build(),
            Parameter::builder()
                .parameter_key("WritePathPrefix")
                .parameter_value(self.write_path_prefix)
                .build(),
            Parameter::builder()
                .parameter_key("RoleName")
                .parameter_value(self.role_name)
                .build(),
            Parameter::builder()
                .parameter_key("Namespace")
                .parameter_value(self.namespace)
                .build(),
            Parameter::builder()
                .parameter_key("ServiceAccountName")
                .parameter_value(self.service_account_name)
                .build(),
        ]
    }
}

pub struct AWSConfigState {
    pub cf_client: Arc<Client>,
    pub cf_config: Arc<SdkConfig>,
}

impl AWSConfigState {
    pub async fn new(region: Region) -> Self {
        let cf_config = Arc::new(aws_config::from_env().region(region).load().await);
        let cf_client = Arc::new(Client::new(&cf_config));
        Self {
            cf_client,
            cf_config,
        }
    }

    pub async fn does_stack_exist(&self, stack_name: &str) -> bool {
        let describe_stacks_result = self
            .cf_client
            .describe_stacks()
            .stack_name(stack_name)
            .send()
            .await;

        match describe_stacks_result {
            Ok(result) => {
                info!("Stack {:?} exists", stack_name);
                result.stacks.is_some()
            }
            Err(_) => false,
        }
    }

    pub async fn create_cloudformation_stack(
        &self,
        stack_name: &str,
        params: &CloudFormationParams,
        cloudformation_template_bucket: String,
        aws_region: String,
    ) -> Result<(), ConductorError> {
        let template_url = format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            cloudformation_template_bucket, aws_region, "conductor-cf-template-v2.yaml"
        );
        let parameters = params.clone().parameters();
        if !self.does_stack_exist(stack_name).await {
            // todo(nhudson): We need to add tags to the stack
            // get with @sjmiller609 to figure out how we want
            // to tag these CF stacks.
            let create_stack_result = self
                .cf_client
                .create_stack()
                .stack_name(stack_name)
                .template_url(template_url)
                .set_parameters(Some(parameters))
                .capabilities(Capability::CapabilityNamedIam)
                .send()
                .await;

            match create_stack_result {
                Ok(result) => {
                    info!("Created stack: {:?}", result.stack_id);
                    Ok(())
                }
                Err(err) => {
                    error!("Error creating stack: {:?}", err);
                    Err(ConductorError::AwsError(Box::new(err.into())))
                }
            }
        } else {
            info!("Stack {:?} already exists, no-op", stack_name);
            Ok(())
        }
    }

    pub async fn delete_cloudformation_stack(
        &self,
        stack_name: &str,
    ) -> Result<(), ConductorError> {
        if self.does_stack_exist(stack_name).await {
            let delete_stack_result = self
                .cf_client
                .delete_stack()
                .stack_name(stack_name)
                .send()
                .await;

            match delete_stack_result {
                Ok(_) => {
                    info!("Deleted stack: {:?}", stack_name);
                    Ok(())
                }
                Err(err) => {
                    error!("Error deleting stack: {:?}", err);
                    Err(ConductorError::AwsError(Box::new(err.into())))
                }
            }
        } else {
            info!("Stack {:?} doesn't exist, no-op", stack_name);
            Ok(())
        }
    }

    // Function to lookup outputs from a specific stack
    pub async fn lookup_cloudformation_stack(
        &self,
        stack_name: &str,
    ) -> Result<(Option<String>, Option<String>), ConductorError> {
        let describe_stacks_result = self
            .cf_client
            .describe_stacks()
            .stack_name(stack_name)
            .send()
            .await;

        match describe_stacks_result {
            Ok(response) => {
                if let Some(stacks) = response.stacks {
                    for stack in stacks {
                        if let Some(stack_status) = stack.stack_status {
                            if stack_status == StackStatus::CreateComplete
                                || stack_status == StackStatus::UpdateComplete
                            {
                                if let Some(outputs) = stack.outputs {
                                    let mut role_name: Option<String> = None;
                                    let mut role_arn: Option<String> = None;
                                    for output in outputs {
                                        match output.output_key.as_deref() {
                                            Some("RoleName") => role_name = output.output_value,
                                            Some("RoleArn") => role_arn = output.output_value,
                                            _ => (),
                                        }
                                    }
                                    return Ok((role_name, role_arn));
                                }
                            }
                        }
                    }
                }
                Err(ConductorError::NoOutputsFound)
            }
            Err(err) => {
                error!("Error describing stack: {:?}", err);
                Err(ConductorError::AwsError(Box::new(err.into())))
            }
        }
    }
}
