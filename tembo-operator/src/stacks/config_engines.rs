use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    apis::postgres_parameters::{ConfigValue, PgConfig},
    errors::ValueError,
    stacks::types::Stack,
};

const DEFAULT_MAINTENANCE_WORK_MEM_MB: i32 = 64;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigEngine {
    Standard,
    OLAP,
    MQ,
}

// The standard configuration engine
// intended to be used as a baseline for other configuration engines
pub fn standard_config_engine(stack: &Stack) -> Vec<PgConfig> {
    let sys_mem_mb = parse_memory(stack).expect("no memory values");
    let sys_storage_gb = parse_storage(stack).expect("no storage values");

    let shared_buffer_val_mb = standard_shared_buffers(sys_mem_mb);
    let max_connections: i32 = standard_max_connections(sys_mem_mb);
    let work_mem = dynamic_work_mem(sys_mem_mb as i32, shared_buffer_val_mb, max_connections);
    let bgwriter_delay_ms = standard_bgwriter_delay_ms(sys_mem_mb as i32);
    let effective_cache_size_mb = dynamic_effective_cache_size_mb(sys_mem_mb as i32);
    let maintenance_work_mem_mb = dynamic_maintenance_work_mem_mb(sys_mem_mb as i32);
    let max_wal_size_gb = dynamic_max_wal_size(sys_storage_gb as i32);

    vec![
        PgConfig {
            name: "shared_buffers".to_owned(),
            value: ConfigValue::Single(format!("{shared_buffer_val_mb}MB")),
        },
        PgConfig {
            name: "max_connections".to_owned(),
            value: ConfigValue::Single(max_connections.to_string()),
        },
        PgConfig {
            name: "work_mem".to_owned(),
            value: ConfigValue::Single(format!("{work_mem}MB")),
        },
        PgConfig {
            name: "bgwriter_delay".to_owned(),
            value: ConfigValue::Single(format!("{bgwriter_delay_ms}ms")),
        },
        PgConfig {
            name: "effective_cache_size".to_owned(),
            value: ConfigValue::Single(format!("{effective_cache_size_mb}MB")),
        },
        PgConfig {
            name: "maintenance_work_mem".to_owned(),
            value: ConfigValue::Single(format!("{maintenance_work_mem_mb}MB")),
        },
        PgConfig {
            name: "max_wal_size".to_owned(),
            value: ConfigValue::Single(format!("{max_wal_size_gb}GB")),
        },
    ]
}

pub fn olap_config_engine(stack: &Stack) -> Vec<PgConfig> {
    let sys_mem_mb = parse_memory(stack).expect("no memory values");
    let sys_storage_gb = parse_storage(stack).expect("no storage values");
    let vcpu = parse_cpu(stack);

    let shared_buffer_val_mb = standard_shared_buffers(sys_mem_mb);
    let max_connections: i32 = olap_max_connections(sys_mem_mb as i32);
    let work_mem = dynamic_work_mem(sys_mem_mb as i32, shared_buffer_val_mb, max_connections);
    let effective_cache_size_mb = dynamic_effective_cache_size_mb(sys_mem_mb as i32);
    let maintenance_work_mem_mb = olap_maintenance_work_mem_mb(sys_mem_mb as i32);
    let max_wal_size_gb: i32 = dynamic_max_wal_size(sys_storage_gb as i32);
    let max_parallel_workers = olap_max_parallel_workers(vcpu);
    let max_parallel_workers_per_gather = olap_max_parallel_workers_per_gather(vcpu);
    let max_worker_processes = olap_max_worker_processes(vcpu);
    vec![
        PgConfig {
            name: "effective_cache_size".to_owned(),
            value: ConfigValue::Single(format!("{effective_cache_size_mb}MB")),
        },
        PgConfig {
            name: "maintenance_work_mem".to_owned(),
            value: ConfigValue::Single(format!("{maintenance_work_mem_mb}MB")),
        },
        PgConfig {
            name: "max_connections".to_owned(),
            value: ConfigValue::Single(max_connections.to_string()),
        },
        PgConfig {
            name: "max_parallel_workers".to_owned(),
            value: ConfigValue::Single(max_parallel_workers.to_string()),
        },
        PgConfig {
            name: "max_parallel_workers_per_gather".to_owned(),
            value: ConfigValue::Single(max_parallel_workers_per_gather.to_string()),
        },
        PgConfig {
            name: "max_wal_size".to_owned(),
            value: ConfigValue::Single(format!("{max_wal_size_gb}GB")),
        },
        PgConfig {
            name: "max_worker_processes".to_owned(),
            value: ConfigValue::Single(max_worker_processes.to_string()),
        },
        PgConfig {
            name: "shared_buffers".to_owned(),
            value: ConfigValue::Single(format!("{shared_buffer_val_mb}MB")),
        },
        PgConfig {
            name: "work_mem".to_owned(),
            value: ConfigValue::Single(format!("{work_mem}MB")),
        },
    ]
}

// the MQ config engine is essentially the standard OLTP config engine, with a few tweaks
pub fn mq_config_engine(stack: &Stack) -> Vec<PgConfig> {
    let sys_mem_mb = parse_memory(stack).expect("no memory values");
    let shared_buffer_val_mb = mq_shared_buffers(sys_mem_mb);

    // start with the output from the standard config engine
    let mut configs = standard_config_engine(stack);

    for config in configs.iter_mut() {
        if config.name == "shared_buffers" {
            config.value = ConfigValue::Single(format!("{shared_buffer_val_mb}MB"))
        }
    }

    configs
}

// olap formula for max_parallel_workers_per_gather
fn olap_max_parallel_workers_per_gather(cpu: i32) -> i32 {
    // higher of default (2) or 0.5 * cpu
    let scaled = i32::max((cpu as f64 * 0.5).floor() as i32, 2);
    // cap at 8
    i32::max(scaled, 8)
}

fn olap_max_parallel_workers(cpu: i32) -> i32 {
    // higher of the default (8) or cpu
    i32::max(8, cpu)
}

fn olap_max_worker_processes(cpu: i32) -> i32 {
    i32::max(1, cpu)
}

// olap formula for maintenance_work_mem
fn olap_maintenance_work_mem_mb(sys_mem_mb: i32) -> i32 {
    // max of the default 64MB and 10% of system memory
    const MAINTENANCE_WORK_MEM_RATIO: f64 = 0.10;
    i32::max(
        DEFAULT_MAINTENANCE_WORK_MEM_MB,
        (sys_mem_mb as f64 * MAINTENANCE_WORK_MEM_RATIO).floor() as i32,
    )
}

// general purpose formula for maintenance_work_mem
fn dynamic_maintenance_work_mem_mb(sys_mem_mb: i32) -> i32 {
    // max of the default 64MB and 5% of system memory
    const MAINTENANCE_WORK_MEM_RATIO: f64 = 0.05;
    const DEFAULT_MAINTENANCE_WORK_MEM_MB: i32 = 64;
    i32::max(
        DEFAULT_MAINTENANCE_WORK_MEM_MB,
        (sys_mem_mb as f64 * MAINTENANCE_WORK_MEM_RATIO).floor() as i32,
    )
}

fn dynamic_max_wal_size(sys_disk_gb: i32) -> i32 {
    // maximum percentage of disk to give to the WAL process
    // TODO: ideal should be: min(20% of disk, f(disk throughput))
    // also, this will panic if < 10GB disk, which is not supported
    if sys_disk_gb < 10 {
        panic!("disk size must be greater than 10GB")
    } else if sys_disk_gb <= 100 {
        (sys_disk_gb as f32 * 0.2).floor() as i32
    } else if sys_disk_gb <= 1000 {
        (sys_disk_gb as f32 * 0.1).floor() as i32
    } else {
        (sys_disk_gb as f32 * 0.05).floor() as i32
    }
}

// piecewise function to set the background writer delay
fn standard_bgwriter_delay_ms(sys_mem_mb: i32) -> i32 {
    // bgwriter_delay = ≥ 8Gb ram - set to 10, set to 200 for everything smaller than 8Gb
    if sys_mem_mb >= 8192 {
        10
    } else {
        200
    }
}

// in olap, we want to limit the number of connections
// never to exceed MAX_CONNECTIONS
fn olap_max_connections(sys_mem_mb: i32) -> i32 {
    const MAX_CONNECTIONS: i32 = 100;
    i32::min(standard_max_connections(sys_mem_mb as f64), MAX_CONNECTIONS)
}

// returns Memory from a Stack in Mb
fn parse_memory(stack: &Stack) -> Result<f64, ValueError> {
    let mem_str = stack
        .infrastructure
        .as_ref()
        .expect("infra required for a configuration engine")
        .memory
        .clone();
    let (mem, unit) = split_string(&mem_str)?;
    match unit.as_str() {
        "Gi" => Ok(mem * 1024.0),
        "Mi" => Ok(mem),
        _ => Err(ValueError::Invalid(format!(
            "Invalid mem value: {}",
            mem_str
        ))),
    }
}

// returns the Storage from a Stack in GB
fn parse_storage(stack: &Stack) -> Result<f64, ValueError> {
    let storage_str = stack
        .infrastructure
        .as_ref()
        .expect("infra required for a configuration engine")
        .storage
        .clone();
    let (storage, unit) = split_string(&storage_str)?;
    match unit.as_str() {
        "Gi" => Ok(storage),
        _ => Err(ValueError::Invalid(format!(
            "Invalid storage value: {}",
            storage_str
        ))),
    }
}

// Standard formula for shared buffers, 25% of system memory
// returns the value as string including units, e.g. 128MB
fn mq_shared_buffers(mem_mb: f64) -> i32 {
    (mem_mb * 0.6).floor() as i32
}

// Standard formula for shared buffers, 25% of system memory
// returns the value as string including units, e.g. 128MB
fn standard_shared_buffers(mem_mb: f64) -> i32 {
    (mem_mb / 4.0_f64).floor() as i32
}

fn standard_max_connections(mem_mb: f64) -> i32 {
    const MEM_PER_CONNECTION_MB: f64 = 9.5;
    (mem_mb / MEM_PER_CONNECTION_MB).floor() as i32
}

// returns work_mem value in MB
fn dynamic_work_mem(sys_mem_mb: i32, shared_buffers_mb: i32, max_connections: i32) -> i32 {
    (((sys_mem_mb - shared_buffers_mb) as f64 - (sys_mem_mb as f64 * 0.2)) / max_connections as f64)
        .floor() as i32
}

// generally safe for most workloads
fn dynamic_effective_cache_size_mb(sys_mem_mb: i32) -> i32 {
    const EFFECTIVE_CACHE_SIZE: f64 = 0.70;
    (sys_mem_mb as f64 * EFFECTIVE_CACHE_SIZE).floor() as i32
}

use lazy_static::lazy_static;

lazy_static! {
    static ref RE: Regex = Regex::new(r"^([0-9]*\.?[0-9]+)([a-zA-Z]+)$").unwrap();
}

use regex::Regex;

fn split_string(input: &str) -> Result<(f64, String), ValueError> {
    if let Some(cap) = RE.captures(input) {
        let num = cap[1].parse::<f64>()?;
        let alpha = cap[2].to_string();
        Ok((num, alpha))
    } else {
        Err(ValueError::Invalid(format!(
            "Invalid string format: {}",
            input
        )))
    }
}

// returns the vCPU count
fn parse_cpu(stack: &Stack) -> i32 {
    stack
        .infrastructure
        .as_ref()
        .expect("infra required for a configuration engine")
        .cpu
        .to_string()
        .parse::<i32>()
        .expect("failed parsing cpu")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stacks::types::*;

    #[test]
    #[should_panic]
    fn test_invalid_storage_dynamic_wal() {
        dynamic_max_wal_size(9);
    }

    #[test]
    fn test_dynamic_max_wall_size() {
        let max_wal_size = dynamic_max_wal_size(100);
        assert_eq!(max_wal_size, 20);
        let max_wal_size = dynamic_max_wal_size(1000);
        assert_eq!(max_wal_size, 100);
        let max_wal_size = dynamic_max_wal_size(3000);
        assert_eq!(max_wal_size, 150);
    }

    #[test]
    fn test_dynamic_maintenance_work_mem_mb() {
        let work_mem = dynamic_maintenance_work_mem_mb(1024);
        assert_eq!(work_mem, 64);
        let work_mem = dynamic_maintenance_work_mem_mb(10240);
        assert_eq!(work_mem, 512);
    }

    #[test]
    fn test_effective_cache_size() {
        let work_mem = dynamic_effective_cache_size_mb(1024);
        assert_eq!(work_mem, 716);
    }

    #[test]
    fn test_standard_bgwriter_delay_ms() {
        let bgwriter_delay = standard_bgwriter_delay_ms(1024);
        assert_eq!(bgwriter_delay, 200);
        let bgwriter_delay = standard_bgwriter_delay_ms(8192);
        assert_eq!(bgwriter_delay, 10);
    }

    #[test]
    fn test_dynamic_work_mem() {
        let work_mem = dynamic_work_mem(1024, 250, 107);
        assert_eq!(work_mem, 5);

        let work_mem = dynamic_work_mem(16384, 4096, 100);
        assert_eq!(work_mem, 90);
    }

    #[test]
    fn test_standard_config_engine() {
        let mut stack = Stack {
            name: "test".to_owned(),
            postgres_config_engine: Some(ConfigEngine::Standard),
            ..Stack::default()
        };
        let infra = Infrastructure {
            cpu: "1".to_string(),
            memory: "16Gi".to_string(),
            storage: "10Gi".to_string(),
        };
        stack.infrastructure = Some(infra);
        let configs = standard_config_engine(&stack);
        assert_eq!(configs[0].name, "shared_buffers");
        assert_eq!(configs[0].value.to_string(), "4096MB");
        assert_eq!(configs[1].name, "max_connections");
        assert_eq!(configs[1].value.to_string(), "1724");
        assert_eq!(configs[2].name, "work_mem");
        assert_eq!(configs[2].value.to_string(), "5MB");
        assert_eq!(configs[3].name, "bgwriter_delay");
        assert_eq!(configs[3].value.to_string(), "10ms");
        assert_eq!(configs[4].name, "effective_cache_size");
        assert_eq!(configs[4].value.to_string(), "11468MB");
        assert_eq!(configs[5].name, "maintenance_work_mem");
        assert_eq!(configs[5].value.to_string(), "819MB");
        assert_eq!(configs[6].name, "max_wal_size");
        assert_eq!(configs[6].value.to_string(), "2GB");
    }

    #[test]
    fn test_standard_shared_buffers() {
        let shared_buff = standard_shared_buffers(1024.0);
        assert_eq!(shared_buff, 256);
        let shared_buff = standard_shared_buffers(10240.0);
        assert_eq!(shared_buff, 2560);

        let shared_buff = mq_shared_buffers(1024.0);
        assert_eq!(shared_buff, 614);

        let shared_buff = mq_shared_buffers(10240.0);
        assert_eq!(shared_buff, 6144);
    }

    #[test]
    fn test_olap_max_connections() {
        // capped at 100
        let max_con = olap_max_connections(4096);
        assert_eq!(max_con, 100);
        let max_con = olap_max_connections(1024);
        assert_eq!(max_con, 100);
    }

    #[test]
    fn test_standard_max_connections() {
        let max_connections = standard_max_connections(1024.0);
        assert_eq!(max_connections, 107);
        let max_connections = standard_max_connections(8192.0);
        assert_eq!(max_connections, 862);
    }

    #[test]
    fn test_split_string() {
        let (mem, unit) = split_string("10Gi").expect("failed parsing val");
        assert_eq!(mem, 10.0);
        assert_eq!(unit, "Gi");

        let error_val = split_string("BadData");
        assert!(error_val.is_err());
        let error_val: Result<(f64, String), ValueError> = split_string("Gi10");
        assert!(error_val.is_err());
    }

    #[test]
    fn test_olap_config_engine() {
        let stack = Stack {
            name: "test".to_owned(),
            compute_templates: None,
            infrastructure: Some(Infrastructure {
                cpu: "4".to_string(),
                memory: "16Gi".to_string(),
                storage: "10Gi".to_string(),
            }),
            app_services: None,
            image: None,
            extensions: None,
            trunk_installs: None,
            description: None,
            stack_version: None,
            postgres_config_engine: Some(ConfigEngine::Standard),
            postgres_config: None,
            postgres_metrics: None,
        };
        let configs = olap_config_engine(&stack);

        assert_eq!(configs[0].name, "effective_cache_size");
        assert_eq!(configs[0].value.to_string(), "11468MB");
        assert_eq!(configs[1].name, "maintenance_work_mem");
        assert_eq!(configs[1].value.to_string(), "1638MB");
        assert_eq!(configs[2].name, "max_connections");
        assert_eq!(configs[2].value.to_string(), "100");
        assert_eq!(configs[3].name, "max_parallel_workers");
        assert_eq!(configs[3].value.to_string(), "8");
        assert_eq!(configs[4].name, "max_parallel_workers_per_gather");
        assert_eq!(configs[4].value.to_string(), "8");
        assert_eq!(configs[5].name, "max_wal_size");
        assert_eq!(configs[5].value.to_string(), "2GB");
        assert_eq!(configs[6].name, "max_worker_processes");
        assert_eq!(configs[6].value.to_string(), "4");
        assert_eq!(configs[7].name, "shared_buffers");
        assert_eq!(configs[7].value.to_string(), "4096MB");
        assert_eq!(configs[8].name, "work_mem");
        assert_eq!(configs[8].value.to_string(), "90MB");
    }
}
