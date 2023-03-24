use controller::apis::coredb_types::CoreDB;
use kube::CustomResourceExt;
fn main() {
    print!("{}", serde_yaml::to_string(&CoreDB::crd()).unwrap())
}
