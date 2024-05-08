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
}

fn main() {
    let args = Args::parse();
    println!("PostgreSQL version: {}", args.pg_version);
    let stack_name = args.stack.to_string();
    let stack = tembo_stacks::stacks::get_stack(args.stack);
    let coredb = stack.to_coredb();
    let yaml = generate_spec(&coredb);
    let filename = format!("{}-coredb.yaml", stack_name);
    std::fs::write(&filename, yaml).expect("Unable to write to file");
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

fn generate_spec(spec: &CoreDBSpec) -> String {
    let kspec = Spec {
        api_version: "coredb.io/v1alpha1".to_string(),
        kind: "CoreDB".to_string(),
        metadata: serde_json::json!({
            "name": spec.stack.as_ref().unwrap().name.to_lowercase(),
        }),
        spec: spec.clone(),
    };
    serde_json::to_string_pretty(&kspec).unwrap()
}
