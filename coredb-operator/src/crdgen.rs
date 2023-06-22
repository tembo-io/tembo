use controller::apis::coredb_types::CoreDB;
use kube::CustomResourceExt;
fn main() {
    let crd_str = serde_yaml::to_string(&CoreDB::crd()).unwrap();
    let st = crd_str.replace("required:\n                - queries", "");
    print!("{}", st)
}
