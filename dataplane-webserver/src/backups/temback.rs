use crate::config::Config;
use actix_web::{error::ErrorInternalServerError, Error};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams};
use tokio::io::AsyncReadExt;

const TEMBACK_INSTALL_DIR: &str = "/var/lib/postgresql/data";
const TEMBACK_INSTALL_CMD_TEMPLATE: &str = "curl -L https://github.com/tembo-io/temback/releases/download/{version}/temback-{version}-linux-amd64.tar.gz | tar -C {install_dir} --strip-components=1 -zxf - temback-{version}-linux-amd64/temback && chmod +x {install_dir}/temback";

/// Install temback binary in the specified pod if it doesn't exist.
///
/// # Arguments
/// * `pods_api` - Kubernetes Pod API
/// * `pod_name` - Name of the pod to install temback in
/// * `config` - Application configuration containing temback version
///
/// # Returns
/// * `Ok(())` if installation succeeds or binary already exists
/// * `Err(Error)` if installation fails
pub async fn install_temback(
    pods_api: &Api<Pod>,
    pod_name: &str,
    config: &Config,
) -> Result<(), Error> {
    let temback_path = format!("{}/temback", TEMBACK_INSTALL_DIR);
    let check_cmd = vec!["ls", temback_path.as_str()];
    let attach_params = AttachParams::default()
        .container("postgres")
        .stderr(true)
        .stdout(true)
        .stdin(false);

    tracing::debug!(
        pod = %pod_name,
        command = ?check_cmd,
        path = %temback_path,
        "Checking for temback binary"
    );

    let mut check_output = pods_api
        .exec(pod_name, check_cmd, &attach_params)
        .await
        .map_err(|e| ErrorInternalServerError(format!("Failed to execute temback check: {}", e)))?;

    let mut error_msg = String::new();
    if let Some(mut stderr) = check_output.stderr() {
        stderr.read_to_string(&mut error_msg).await.map_err(|e| {
            ErrorInternalServerError(format!("Failed to read check command error output: {}", e))
        })?;
    }

    if !error_msg.is_empty() {
        tracing::info!(
            pod = %pod_name,
            version = %config.temback_version,
            "temback not found, installing..."
        );

        let install_cmd_str = TEMBACK_INSTALL_CMD_TEMPLATE
            .replace("{version}", &config.temback_version)
            .replace("{install_dir}", TEMBACK_INSTALL_DIR);
        let install_cmd = vec!["sh", "-c", &install_cmd_str];

        tracing::debug!(
            pod = %pod_name,
            command = ?install_cmd,
            "Installing temback"
        );

        let mut install_output = pods_api
            .exec(pod_name, install_cmd, &attach_params)
            .await
            .map_err(|e| ErrorInternalServerError(format!("Failed to install temback: {}", e)))?;

        // Wait for install to finish by reading all output
        let mut _out = String::new();
        if let Some(mut stdout) = install_output.stdout() {
            stdout.read_to_string(&mut _out).await.ok();
        }
        let mut _err = String::new();
        if let Some(mut stderr) = install_output.stderr() {
            stderr.read_to_string(&mut _err).await.ok();
        }

        // Verify the binary exists after installation
        let check_cmd = vec!["ls", temback_path.as_str()];
        let mut verify_result = pods_api
            .exec(pod_name, check_cmd, &attach_params)
            .await
            .map_err(|e| {
                ErrorInternalServerError(format!("Failed to verify temback installation: {}", e))
            })?;

        let mut verify_error = String::new();
        if let Some(mut stderr) = verify_result.stderr() {
            stderr
                .read_to_string(&mut verify_error)
                .await
                .map_err(|e| {
                    ErrorInternalServerError(format!(
                        "Failed to read verification error output: {}",
                        e
                    ))
                })?;
        }

        if !verify_error.is_empty() {
            return Err(ErrorInternalServerError(
                "Failed to install temback: Binary not found after installation".to_string(),
            ));
        }

        tracing::info!(
            pod = %pod_name,
            "Successfully installed temback"
        );
    }

    Ok(())
}
