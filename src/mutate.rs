use actix_web::{post, web, HttpResponse, Responder};
use json_patch::{diff, Patch};
use k8s_openapi::api::core::v1::Pod;
use kube::core::{
    admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    TypeMeta,
};
use serde_json::json;
use tracing::{debug, error};

use crate::{config::Config, container::create_init_container};

#[post("/mutate")]
async fn mutate(
    body: web::Json<AdmissionReview<Pod>>,
    config: web::Data<Config>,
) -> impl Responder {
    // Extract the AdmissionRequest from the AdmissionReview
    let admission_request: AdmissionRequest<Pod> = body.clone().request.unwrap();

    // Check for the kind of resource in the AdmissionRequest, we only
    // care about Pod resources
    if !admission_request.kind.group.is_empty()
        || admission_request.kind.version != "v1"
        || admission_request.kind.kind != "Pod"
    {
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
        Some(request) => request.object.as_ref(),
        None => {
            return HttpResponse::BadRequest().body("expected AdmissionRequest");
        }
    };

    let pod = match pod {
        Some(pod) => pod,
        None => {
            return HttpResponse::BadRequest().body("expected pod object");
        }
    };

    if !pod
        .metadata
        .annotations
        .as_ref()
        .map_or(false, |annotations| {
            annotations.contains_key(&config.pod_annotation)
        })
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
        return HttpResponse::Ok().json(AdmissionReview {
            response: Some(mk_allow_response(&admission_request, None)),
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
        let init_container = create_init_container(&config);
        spec.init_containers
            .get_or_insert_with(Vec::new)
            .push(init_container);
    } else {
        error!(
            "Pod spec is missing, cannot inject initContainer: {:?}",
            pod.clone()
        );
    };

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
        for volume in volumes {
            if required_volumes.contains(&volume.name.as_str()) {
                return true;
            }
        }
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

pub fn mk_deny_response(ar: &AdmissionRequest<Pod>, message: &str) -> AdmissionResponse {
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
