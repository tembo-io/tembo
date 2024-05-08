use clap::Parser;
use tembo_controller::apis::coredb_types::CoreDBSpec;
use tembo_stacks::stacks::types::StackType;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = 16)]
    pg_version: i32,

    #[arg(long)]
    stack: StackType,

    #[arg(long)]
    name: Option<String>,
}

fn main() {
    let args = Args::parse();
    let resource_name = match args.name {
        Some(name) => name.to_lowercase(),
        None => args.stack.to_string().to_lowercase(),
    };
    let stack_name = args.stack.to_string();
    let stack = tembo_stacks::stacks::get_stack(args.stack);
    let coredb = stack.to_coredb("1".to_string(), "1Gi".to_string(), "10Gi".to_string());
    let json = generate_spec(&coredb, &resource_name);
    // writing to json because not an easy way to string quote nested postgres config values in yaml
    // but serializing as json handles this
    let filename = format!("{resource_name}-{stack_name}-coredb.json");
    std::fs::write(&filename, json).expect("Unable to write to file");
    println!("Wrote to spec: {}", filename);
}

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Spec {
    api_version: String,
    kind: String,
    metadata: serde_json::Value,
    spec: CoreDBSpec,
}

fn generate_spec(spec: &CoreDBSpec, resource_name: &str) -> String {
    let kspec = Spec {
        api_version: "coredb.io/v1alpha1".to_string(),
        kind: "CoreDB".to_string(),
        metadata: serde_json::json!({
            "name": resource_name,
        }),
        spec: spec.clone(),
    };
    serde_json::to_string_pretty(&kspec).unwrap()
}
