use kube::CustomResourceExt;
use controller::apis::coredb_types::CoreDB;
fn main() {
    print!("{}", serde_yaml::to_string(&CoreDB::crd()).unwrap())
}
