use actix_web::{post, web, HttpResponse, Responder};
use json_patch::{diff, Patch};
use k8s_openapi::api::core::v1::{Pod, VolumeMount};
use kube::core::{
    admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    TypeMeta,
};
use kube::Client;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tembo_telemetry::TelemetryConfig;
use tokio::sync::RwLock;
use tracing::*;

use crate::{config::Config, container::*};

#[instrument(skip(client), fields(trace_id))]
#[post("/mutate")]
async fn mutate(
    body: web::Json<AdmissionReview<Pod>>,
    config: web::Data<Config>,
    namespaces: web::Data<Arc<RwLock<HashSet<String>>>>,
    client: web::Data<Arc<Client>>,
    tc: web::Data<TelemetryConfig>,
) -> impl Responder {
    // Set trace_id for logging
    let trace_id = tc.get_trace_id();
    Span::current().record("trace_id", field::display(&trace_id));

    // Extract the AdmissionRequest from the AdmissionReview
    let admission_request: AdmissionRequest<Pod> = body.clone().request.unwrap();

    // Check if the namespace is in the list of namespaces to watch
    let namespace = admission_request.namespace.as_ref().unwrap();

    if !namespaces.read().await.contains(namespace) {
        debug!(
            "Namespace {} is not in the list of namespaces to watch",
            namespace
        );
        return HttpResponse::Ok().json(AdmissionReview {
            response: Some(mk_allow_response(&admission_request, None)),
            request: Some(admission_request),
            types: TypeMeta {
                api_version: "admission.k8s.io/v1".to_string(),
                kind: "AdmissionReview".to_string(),
            },
        });
    }

    // Check for the kind of resource in the AdmissionRequest, we only
    // care about Pod resources
    if !admission_request.kind.group.is_empty()
        || admission_request.kind.version != "v1"
        || admission_request.kind.kind != "Pod"
    {
        debug!(
            "Skipping resource with group: {}, version: {}, kind: {}",
            admission_request.kind.group,
            admission_request.kind.version,
            admission_request.kind.kind
        );
        return HttpResponse::Ok().json(AdmissionReview {
            response: Some(mk_allow_response(&admission_request, None)),
            request: Some(admission_request),
            types: TypeMeta {
                api_version: "admission.k8s.io/v1".to_string(),
                kind: "AdmissionReview".to_string(),
            },
        });
    }

    // Extract the Pod from the AdmissionRequest
    let ar: AdmissionReview<Pod> = body.into_inner();
    let pod: Option<&Pod> = match &ar.request {
        Some(request) => {
            debug!("Got AdmissionRequest: {:?}", ar.request);
            request.object.as_ref()
        }
        None => {
            return HttpResponse::BadRequest().body("expected AdmissionRequest");
        }
    };

    let pod = match pod {
        Some(pod) => {
            debug!("Got Pod: {:?}", pod);
            pod
        }
        None => {
            return HttpResponse::BadRequest().body("expected pod object");
        }
    };

    // Extract cluster name from the pod labels
    let cluster_name = pod
        .metadata
        .labels
        .as_ref()
        .and_then(|labels| labels.get("cnpg.io/cluster"))
        .map(|s| s.to_string());

    if !pod
        .metadata
        .annotations
        .as_ref()
        .is_some_and(|annotations| annotations.contains_key(&config.pod_annotation))
    {
        return match ar.request {
            Some(request) => HttpResponse::Ok().json(AdmissionReview {
                response: Some(mk_allow_response(&request, None)),
                request: Some(request),
                types: TypeMeta {
                    api_version: "admission.k8s.io/v1".to_string(),
                    kind: "AdmissionReview".to_string(),
                },
            }),
            None => HttpResponse::BadRequest().body("expected AdmissionRequest"),
        };
    }

    // Check if the pod has all required volumes
    let required_volumes = vec!["pgdata", "scratch-data"];
    if !has_required_volumes(pod, &required_volumes) {
        error!(
            "Pod spec does not contain all required volumes, will not mutate: {:?}",
            pod
        );
        // set message to say that the pod does not have all required volumes
        let message = "Pod spec does not contain all required volumes, will not mutate";
        return HttpResponse::Ok().json(AdmissionReview {
            response: Some(mk_deny_response(&admission_request, message)),
            request: Some(admission_request),
            types: TypeMeta {
                api_version: "admission.k8s.io/v1".to_string(),
                kind: "AdmissionReview".to_string(),
            },
        });
    }

    // At this point, the Pod has the expected annotation.
    // So we can inject or patch the initContainer into it.
    let mut new_pod = pod.clone();
    if let Some(spec) = &mut new_pod.spec {
        // Check to make sure we don't add the initContainer more than once
        if spec
            .init_containers
            .as_ref()
            .is_some_and(|init_containers| {
                init_containers
                    .iter()
                    .any(|c| c.name == config.init_container_name)
            })
        {
            debug!(
                "Pod already has initContainer, skipping: {:?}",
                config.init_container_name.to_string()
            );
        } else {
            let init_container =
                create_init_container(&config, &client, namespace, &cluster_name.unwrap()).await;
            let init_containers = spec.init_containers.take().unwrap_or_default();
            let mut new_init_containers = vec![init_container];
            new_init_containers.extend(init_containers);
            spec.init_containers = Some(new_init_containers);
        }
    } else {
        error!(
            "Pod spec is missing, cannot inject initContainer: {:?}",
            pod.clone()
        );
    };

    // Mutate a Pod when the container name is Postgres and add a scratch
    // volume mounted to /tmp
    if let Some(spec) = &mut new_pod.spec {
        // Iterate over containers to find the 'postgres' container.
        if let Some(postgres_container) = spec.containers.iter_mut().find(|c| c.name == "postgres")
        {
            let volume_mount = VolumeMount {
                mount_path: "/tmp".to_string(),
                mount_propagation: None,
                name: "scratch-data".to_string(),
                // You can leave the other fields as None if you don't need them.
                sub_path: None,
                read_only: None,
                sub_path_expr: None,
            };

            add_volume_mounts(postgres_container, volume_mount);
        } else {
            warn!("Postgres container not found");
        }
    }

    // Calculate patch and add it to the AdmissionResponse
    let patch = generate_pod_patch(pod, &new_pod);

    // Construct and return the AdmissionReview containing the AdmissionResponse.
    let admission_response = match patch {
        Some(patch) => mk_allow_response(&admission_request, Some(patch)),
        None => mk_allow_response(&admission_request, None),
    };
    debug!("AdmissionResponse: {:?}", admission_response);

    HttpResponse::Ok().json(AdmissionReview {
        response: Some(admission_response),
        request: Some(admission_request),
        types: TypeMeta {
            api_version: "admission.k8s.io/v1".to_string(),
            kind: "AdmissionReview".to_string(),
        },
    })
}

// Check to make sure pods have all required volumes
fn has_required_volumes(pod: &Pod, required_volumes: &[&str]) -> bool {
    if let Some(volumes) = &pod.spec.as_ref().unwrap().volumes {
        let existing_volumes: HashSet<_> = volumes.iter().map(|v| v.name.as_str()).collect();
        return required_volumes
            .iter()
            .all(|required_volume| existing_volumes.contains(*required_volume));
    }
    false
}

// This function creates an AdmissionResponse that allows the AdmissionRequest without any modifications.
fn mk_allow_response(ar: &AdmissionRequest<Pod>, patch: Option<Patch>) -> AdmissionResponse {
    let mut response = AdmissionResponse::from(ar);

    if let Some(patch) = patch {
        debug!("Applying patch: {:?}", patch);
        response = response.with_patch(patch).unwrap();
    }

    debug!("Returning response: {:?}", response);
    response
}

fn mk_deny_response(ar: &AdmissionRequest<Pod>, message: &str) -> AdmissionResponse {
    AdmissionResponse::from(ar).deny(message)
}

// Calculate the patch needed to mutate the Pod
fn generate_pod_patch(pod: &Pod, new_pod: &Pod) -> Option<Patch> {
    let op = json!(pod);
    let np = json!(new_pod);

    let patch = diff(&op, &np);
    debug!("Calculated patch: {:?}", patch);

    if patch.is_empty() {
        None
    } else {
        Some(Patch(patch.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use crate::mutate::has_required_volumes;
    use k8s_openapi::api::core::v1::{Pod, PodSpec, Volume};

    #[test]
    fn test_has_required_volumes() {
        let pod = Pod {
            spec: Some(PodSpec {
                volumes: Some(vec![
                    Volume {
                        name: "pgdata".to_string(),
                        ..Default::default()
                    },
                    Volume {
                        name: "scratch-data".to_string(),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let rv = vec!["pgdata", "scratch-data"];

        let result = has_required_volumes(&pod, &rv);

        assert!(result, "Pod should have all required volumes");
    }

    // It's almost impossible to test the other functions here since the the
    // types like AdmissionRequest, AdmissionResponse, PodSpec, etc all have
    // private fields.  We would need to mock the entire Kubernetes API to test
    // them.  For now we are not going to test them.
}
