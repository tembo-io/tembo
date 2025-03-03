use itertools::Itertools;

use schemars::{
    schema::{Schema, SchemaObject},
    JsonSchema,
};
use serde::{
    de::{Error, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{cmp::Ordering, collections::BTreeSet, fmt, str::FromStr};
use thiserror::Error;
use tracing::*;
use utoipa::ToSchema;

// these values are multi-valued, and need to be merged across configuration layers
pub const MULTI_VAL_CONFIGS: [&str; 5] = [
    "shared_preload_libraries",
    "local_preload_libraries",
    "session_preload_libraries",
    "log_destination",
    "search_path",
];

// This array defines the priority order for any multi-value config
// This defines any required order for shared_preload_libraries, otherwise alphabetical
// TODO: move this to a trunk endpoint
pub const MULTI_VAL_CONFIGS_PRIORITY_LIST: [&str; 3] =
    ["citus", "pg_stat_statements", "pg_stat_kcache"];

// configurations that are not allowed to be set by the user
pub const DISALLOWED_CONFIGS: [&str; 65] = [
    "allow_system_table_mods",
    "archive_cleanup_command",
    "archive_command",
    "archive_mode",
    "bonjour",
    "bonjour_name",
    "cluster_name",
    "config_file",
    "data_directory",
    "data_sync_retry",
    "event_source",
    "external_pid_file",
    "full_page_writes",
    "hba_file",
    "hot_standby",
    "ident_file",
    "jit_provider",
    "listen_addresses",
    "log_destination",
    "log_directory",
    "log_file_mode",
    "log_filename",
    "log_rotation_age",
    "log_rotation_size",
    "log_truncate_on_rotation",
    "logging_collector",
    "port",
    "primary_conninfo",
    "primary_slot_name",
    "promote_trigger_file",
    "recovery_end_command",
    "recovery_min_apply_delay",
    "recovery_target",
    "recovery_target_action",
    "recovery_target_inclusive",
    "recovery_target_lsn",
    "recovery_target_name",
    "recovery_target_time",
    "recovery_target_timeline",
    "recovery_target_xid",
    "restart_after_crash",
    "restore_command",
    "ssl",
    "ssl_ca_file",
    "ssl_cert_file",
    "ssl_ciphers",
    "ssl_crl_file",
    "ssl_dh_params_file",
    "ssl_ecdh_curve",
    "ssl_key_file",
    "ssl_max_protocol_version",
    "ssl_passphrase_command",
    "ssl_passphrase_command_supports_reload",
    "ssl_prefer_server_ciphers",
    "stats_temp_directory",
    "synchronous_standby_names",
    "syslog_facility",
    "syslog_ident",
    "syslog_sequence_numbers",
    "syslog_split_messages",
    "unix_socket_directories",
    "unix_socket_group",
    "unix_socket_permissions",
    "wal_level",
    "wal_log_hints",
];

pub const TEMBO_POSTGRESQL_CONF: &str = "tembo.postgresql.conf";
pub const TEMBO_POSTGRESQL_CONF_VOLUME_PATH: &str = "/tembo/config";
pub const TEMBO_POSTGRESQL_CONFIGMAP: &str = "tembo-postgresql-conf";

/// PgConfig allows a user to define postgresql configuration settings in the
/// postgres configuration file.  This is a subset of the postgresql.conf
/// configuration settings.  The full list of settings that we currently **DO NOT**
/// support can be found [here](https://cloudnative-pg.io/documentation/1.20/postgresql_conf/#fixed-parameters).
///
/// **Example**: The following example shows how to set the `max_connections`,
/// `shared_buffers` and `max_wal_size` configuration settings.
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db
/// spec:
///   runtime_config:
///     - name: max_connections
///       value: 100
///     - name: shared_buffers
///       value: 2048MB
///     - name: max_wal_size
///       value: 2GB
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema, ToSchema)]
pub struct PgConfig {
    /// The name of the Postgres configuration parameter.
    pub name: String,

    // The value of the Postgres configuration parameter.
    pub value: ConfigValue,
}

impl PgConfig {
    // converts the configuration to the postgres format
    pub fn to_postgres(&self) -> String {
        format!("{} = '{}'", self.name, self.value)
    }
}

#[derive(Error, Debug)]
pub enum MergeError {
    #[error("SingleValError")]
    SingleValueNotAllowed,
}

fn sort_multivalue_configs(values: &mut [String], priorities: &[&str]) {
    values.sort_unstable_by(|a, b| {
        let a_index = priorities.iter().position(|x| x == a);
        let b_index = priorities.iter().position(|x| x == b);

        match (a_index, b_index) {
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.cmp(b),
        }
    });
}

impl ConfigValue {
    fn combine(self, other: Self) -> Result<Self, MergeError> {
        match (self, other) {
            (ConfigValue::Single(_), _) | (_, ConfigValue::Single(_)) => {
                Err(MergeError::SingleValueNotAllowed)
            }
            (ConfigValue::Multiple(mut set1), ConfigValue::Multiple(mut set2)) => {
                set1.append(&mut set2);
                Ok(ConfigValue::Multiple(set1))
            }
        }
    }
}

pub fn merge_pg_configs(
    vec1: &[PgConfig],
    vec2: &[PgConfig],
    name: &str,
) -> Result<Option<PgConfig>, MergeError> {
    let config1 = vec1.iter().find(|config| config.name == name).cloned();
    let config2 = vec2.iter().find(|config| config.name == name).cloned();
    match (config1, config2) {
        (Some(mut c1), Some(c2)) => match c1.value.combine(c2.value) {
            Ok(combined_value) => {
                c1.value = combined_value;
                Ok(Some(c1))
            }
            Err(e) => Err(e),
        },
        (Some(c), None) | (None, Some(c)) => {
            debug!("No configs to merge");
            Ok(Some(c))
        }
        (None, None) => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ToSchema)]
pub enum ConfigValue {
    Single(String),
    Multiple(BTreeSet<String>),
}

use serde_json::{Error as JsonParsingError, Value};

use serde_json;

pub struct WrapValue(Value);

impl WrapValue {
    fn as_str(&self) -> Option<&str> {
        self.0.as_str()
    }
}

impl From<WrapValue> for Result<ConfigValue, JsonParsingError> {
    fn from(value: WrapValue) -> Self {
        if let Some(s) = value.as_str() {
            if s.contains(',') {
                let set: BTreeSet<String> = s.split(',').map(|s| s.trim().to_string()).collect();
                Ok(ConfigValue::Multiple(set))
            } else {
                Ok(ConfigValue::Single(s.to_string()))
            }
        } else {
            Err(JsonParsingError::custom("Invalid value: expected string"))
        }
    }
}

impl From<&str> for ConfigValue {
    fn from(item: &str) -> Self {
        let values: Vec<String> = item.split(',').map(|s| s.trim().to_string()).collect();
        if values.len() > 1 {
            ConfigValue::Multiple(values.into_iter().collect())
        } else {
            ConfigValue::Single(values[0].clone())
        }
    }
}

impl JsonSchema for ConfigValue {
    fn schema_name() -> String {
        "ConfigValue".to_string()
    }

    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema_object = SchemaObject::default();
        schema_object.metadata().description =
            Some("A postgresql.conf configuration value".to_owned());
        schema_object.metadata().read_only = false;
        // overriding the enums to be a string
        schema_object.instance_type = Some(schemars::schema::InstanceType::String.into());
        Schema::Object(schema_object)
    }
}

impl std::fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigValue::Single(value) => write!(f, "{}", value),
            ConfigValue::Multiple(values) => {
                let mut configs = values.iter().cloned().collect::<Vec<String>>();
                sort_multivalue_configs(&mut configs, &MULTI_VAL_CONFIGS_PRIORITY_LIST);
                let joined_values = configs.join(",");
                write!(f, "{}", joined_values)
            }
        }
    }
}

impl FromStr for ConfigValue {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(',') {
            Ok(ConfigValue::Multiple(
                s.split(',').map(|s| s.to_string()).collect(),
            ))
        } else {
            Ok(ConfigValue::Single(s.to_string()))
        }
    }
}

impl Serialize for ConfigValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ConfigValue::Single(val) => serializer.serialize_str(val),
            ConfigValue::Multiple(set) => {
                let joined = set.iter().join(",");
                serializer.serialize_str(&joined)
            }
        }
    }
}

// we need a special enum to handle deserializing PgConfig
// rust wants to map the the serde_json::Value keys to String,
// but serde_json expects &str
// unable to figure out how to get around this, but mapping
// to enum does the trick.
#[derive(Debug, PartialEq, Eq)]
enum KeyValue {
    Name,
    Value,
}

impl<'de> Deserialize<'de> for KeyValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeyValueVisitor;

        impl Visitor<'_> for KeyValueVisitor {
            type Value = KeyValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("`name` or `value`")
            }

            fn visit_str<E>(self, value: &str) -> Result<KeyValue, E>
            where
                E: Error,
            {
                match value {
                    "name" => Ok(KeyValue::Name),
                    "value" => Ok(KeyValue::Value),
                    _ => Err(Error::unknown_field(value, &["name", "value"])),
                }
            }
        }

        deserializer.deserialize_identifier(KeyValueVisitor)
    }
}

impl<'de> Deserialize<'de> for PgConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PgConfigVisitor;

        impl<'de> Visitor<'de> for PgConfigVisitor {
            type Value = PgConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct PgConfig")
            }

            fn visit_map<M>(self, mut map: M) -> Result<PgConfig, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut name: Option<String> = None;
                let mut value: Option<String> = None;

                // normally these would be &str values to match on
                // but when these are serde_json::Value, they are always String
                // see note related to the KeyValue enum
                while let Some(key) = map.next_key()? {
                    match key {
                        KeyValue::Name => {
                            if name.is_some() {
                                return Err(Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        KeyValue::Value => {
                            if value.is_some() {
                                return Err(Error::duplicate_field("value"));
                            }
                            value = Some(map.next_value()?);
                        }
                    }
                }
                let name = name.ok_or_else(|| M::Error::custom("key 'name' not found"))?;
                let raw_value = value.ok_or_else(|| M::Error::custom("key 'value' not found"))?;

                let value = if MULTI_VAL_CONFIGS.contains(&name.as_str()) {
                    let set: BTreeSet<String> =
                        raw_value.split(',').map(|s| s.trim().to_string()).collect();
                    ConfigValue::Multiple(set)
                } else {
                    ConfigValue::Single(raw_value)
                };

                Ok(PgConfig { name, value })
            }
        }

        const FIELDS: &[&str] = &["name", "value"];
        deserializer.deserialize_struct("PgConfig", FIELDS, PgConfigVisitor)
    }
}

#[cfg(test)]
mod pg_param_tests {
    use super::*;
    use crate::apis::coredb_types::{CoreDBSpec, Stack};
    use std::collections::BTreeMap;

    #[test]
    fn test_pg_config() {
        let pg_config = PgConfig {
            name: "max_parallel_workers".to_string(),
            value: "32".parse().unwrap(),
        };
        assert_eq!(pg_config.to_postgres(), "max_parallel_workers = '32'");
        let pg_config_multi = PgConfig {
            name: "shared_preload_libraries".to_string(),
            value: "pg_cron,pg_stat_statements".parse().unwrap(),
        };
        assert_eq!(
            pg_config_multi.to_postgres(),
            "shared_preload_libraries = 'pg_stat_statements,pg_cron'"
        );
    }

    #[test]
    fn test_get_configs() {
        // test shared_preload_libs get 'merged' appropriately
        let mut set = BTreeSet::new();
        set.insert("pg_partman_bgw".to_string());
        let spec = CoreDBSpec {
            runtime_config: Some(vec![
                PgConfig {
                    name: "shared_buffers".to_string(),
                    value: "0.5GB".parse().unwrap(),
                },
                PgConfig {
                    name: "shared_preload_libraries".to_string(),
                    value: ConfigValue::Multiple(set),
                },
            ]),
            stack: Some(Stack {
                name: "tembo".to_string(),
                postgres_config: Some(vec![
                    PgConfig {
                        name: "pg_stat_statements.track".to_string(),
                        value: "all".parse().unwrap(),
                    },
                    PgConfig {
                        name: "shared_preload_libraries".to_string(),
                        value: "pg_cron,pg_stat_statements".parse().unwrap(),
                    },
                    // and a disallowed config. this must be filtered out
                    PgConfig {
                        name: "log_destination".to_string(),
                        value: "yolo".parse().unwrap(),
                    },
                ]),
            }),
            ..Default::default()
        };
        let mut requires_load: BTreeMap<String, String> = BTreeMap::new();
        requires_load.insert("pg_cron".to_string(), "pg_cron".to_string());
        requires_load.insert(
            "pg_stat_statements".to_string(),
            "pg_stat_statements".to_string(),
        );
        let pg_configs = spec
            .get_pg_configs(requires_load)
            .expect("failed to get pg configs")
            .expect("expected configs");

        println!("pg_configs:  {:?}", pg_configs);
        // assert 3. shared_preload_libraries is merged. log_destination is filtered out
        // pg_stat_statements, shared_buffers, and shared_preload_libraries remain
        assert_eq!(pg_configs.len(), 4);
        assert_eq!(pg_configs[0].name, "pg_stat_statements.track");
        assert_eq!(pg_configs[0].value.to_string(), "all");
        assert_eq!(pg_configs[1].name, "shared_buffers");
        assert_eq!(pg_configs[1].value.to_string(), "0.5GB");
        assert_eq!(pg_configs[2].name, "shared_preload_libraries");
        assert_eq!(
            pg_configs[2].value.to_string(),
            "pg_stat_statements,pg_cron,pg_partman_bgw"
        );
    }

    #[test]
    fn test_alpha_order_multiple() {
        // assert ordering of multi values is according to the priority list
        // values not in the priority list are sorted alphabetically, and go at the end
        let pgc = PgConfig {
            name: "test_configuration".to_string(),
            value: "pg_stat_kcache,pg_stat_statements,a,b,c".parse().unwrap(),
        };
        assert_eq!(
            pgc.to_postgres(),
            "test_configuration = 'pg_stat_statements,pg_stat_kcache,a,b,c'"
        );
        let pgc = PgConfig {
            name: "test_configuration".to_string(),
            value: "a,z,c,pg_stat_kcache,pg_stat_statements".parse().unwrap(),
        };
        println!("pgc: {:?}", pgc);
        println!("pgcval: {:?}", pgc.to_postgres());
        assert_eq!(
            pgc.to_postgres(),
            "test_configuration = 'pg_stat_statements,pg_stat_kcache,a,c,z'"
        );
        let pgc = PgConfig {
            name: "test_configuration".to_string(),
            value: "pg_stat_statments,z,y,x".parse().unwrap(),
        };
        assert_eq!(
            pgc.to_postgres(),
            "test_configuration = 'pg_stat_statments,x,y,z'"
        );
    }

    #[test]
    fn test_merge_pg_configs() {
        let pgc_0 = PgConfig {
            name: "test_configuration".to_string(),
            value: "a,b,c".parse().unwrap(),
        };
        let pgc_1 = PgConfig {
            name: "test_configuration".to_string(),
            value: "x,y,z".parse().unwrap(),
        };

        let merged = merge_pg_configs(&[pgc_0], &[pgc_1], "test_configuration")
            .expect("failed to merge pg configs")
            .expect("expected configs");
        assert_eq!(merged.value.to_string(), "a,b,c,x,y,z");

        // Single value should not be allowed to be merged
        let pgc_0 = PgConfig {
            name: "test_configuration".to_string(),
            value: "a".parse().unwrap(),
        };
        let pgc_1 = PgConfig {
            name: "test_configuration".to_string(),
            value: "b".parse().unwrap(),
        };
        let merged = merge_pg_configs(&[pgc_0], &[pgc_1], "test_configuration");
        assert!(merged.is_err())
    }

    #[test]
    fn test_serialization() {
        // assert a PgConfig can be serialized and deserialized
        let pgc = PgConfig {
            name: "shared_preload_libraries".to_string(),
            value: "a,b,c".parse().unwrap(),
        };
        match pgc.clone().value {
            ConfigValue::Multiple(set) => {
                assert_eq!(set.len(), 3);
                assert!(set.contains("a"));
                assert!(set.contains("b"));
                assert!(set.contains("c"));
            }
            ConfigValue::Single(_) => panic!("expected multiple values"),
        }
        let serialized: String = serde_json::to_string(&pgc).expect("failed to serialize");
        assert_eq!(
            serialized,
            "{\"name\":\"shared_preload_libraries\",\"value\":\"a,b,c\"}"
        );
        let deserialized: PgConfig =
            serde_json::from_str(&serialized).expect("failed to deserialize");
        match deserialized.value {
            ConfigValue::Multiple(set) => {
                assert_eq!(set.len(), 3);
                assert!(set.contains("a"));
                assert!(set.contains("b"));
                assert!(set.contains("c"));
            }

            ConfigValue::Single(_) => panic!("expected multiple values"),
        }
        // a single val, in a MULTI_VAL_CONFIGS is still a ConfigValue::Multiple
        let raw = "{\"name\":\"shared_preload_libraries\",\"value\":\"a\"}";
        let deserialized: PgConfig = serde_json::from_str(raw).expect("failed to deserialize");
        match deserialized.value {
            ConfigValue::Multiple(set) => {
                assert_eq!(set.len(), 1);
                assert!(set.contains("a"));
            }

            ConfigValue::Single(_) => panic!("expected multiple values"),
        }

        // a single val, in MULTI_VAL_CONFIGS is still a ConfigValue::Multiple
        let raw = "{\"name\":\"shared_preload_libraries\",\"value\":\"a\"}";
        let deserialized: PgConfig = serde_json::from_str(raw).expect("failed to deserialize");
        match deserialized.value {
            ConfigValue::Multiple(set) => {
                assert_eq!(set.len(), 1);
                assert!(set.contains("a"));
            }

            ConfigValue::Single(_) => panic!("expected multiple values"),
        }

        // a single val is a ConfigValue::Single
        let raw = "{\"name\":\"shared_buffers\",\"value\":\"1GB\"}";
        let deserialized: PgConfig = serde_json::from_str(raw).expect("failed to deserialize");
        match deserialized.value {
            ConfigValue::Single(s) => {
                assert_eq!(s, "1GB");
            }

            ConfigValue::Multiple(_) => panic!("expected single value"),
        }

        // a known single val, with a comma in value, is still a ConfigValue::Single
        let raw = "{\"name\":\"shared_buffers\",\"value\":\"1GB,2GB\"}";
        let deserialized: PgConfig = serde_json::from_str(raw).expect("failed to deserialize");
        match deserialized.value {
            ConfigValue::Single(s) => {
                assert_eq!(s, "1GB,2GB");
            }

            ConfigValue::Multiple(_) => panic!("expected single value"),
        }

        // from json
        let js = serde_json::json!({
            "name": "shared_preload_libraries",
            "value": "a,b,c"
        });
        let deserialized: PgConfig = serde_json::from_value(js).expect("failed to deserialize");
        match deserialized.value {
            ConfigValue::Multiple(set) => {
                assert_eq!(set.len(), 3);
                assert!(set.contains("a"));
                assert!(set.contains("b"));
                assert!(set.contains("c"));
            }

            ConfigValue::Single(_) => panic!("expected multiple values"),
        }
        // a single with comma still parsed as a single
        let js = serde_json::json!({
            "name": "random_single",
            "value": "a,b,c"
        });
        let deserialized: PgConfig = serde_json::from_value(js).expect("failed to deserialize");
        match deserialized.value {
            ConfigValue::Multiple(_) => panic!("expected single value"),
            ConfigValue::Single(s) => assert_eq!(s, "a,b,c"),
        }
    }
}
