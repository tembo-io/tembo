use controller::apis::coredb_types::CoreDB;
use kube::CustomResourceExt;
fn main() {
    let mut crd = CoreDB::crd();

    // Ensure metadata exists
    if crd.metadata.annotations.is_none() {
        crd.metadata.annotations = Some(Default::default());
    }

    // Add an annotation
    if let Some(annotations) = crd.metadata.annotations.as_mut() {
        annotations.insert("helm.sh/resource-policy".to_string(), "keep".to_string());
    }

    let crd_str = serde_yaml::to_string(&crd).unwrap();
    let st = crd_str.replace("required:\n                - queries", "");
    let prepend_string =
        "{{- if (index .Values \"controller\").enabled }}\n{{- if (index .Values \"controller\").crds.create }}\n";
    let append_string = "{{- end }}\n{{- end }}";
    print!("{}{}{}", prepend_string, st, append_string)
}
