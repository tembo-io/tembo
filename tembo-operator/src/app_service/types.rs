use k8s_openapi::api::core::v1::ResourceRequirements;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;
// defines a app container
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct AppService {
    pub name: String,
    pub image: String,
    pub args: Option<Vec<String>>,
    pub command: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    // PortMapping is in format of String "host:container"
    pub ports: Option<Vec<PortMapping>>,
    pub resources: Option<ResourceRequirements>,
    pub probes: Option<Probes>,
    pub metrics: Option<Metrics>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct PortMapping {
    pub host: u16,
    pub container: u16,
}


// attempting to keep the CRD clean
// this enables ports to be defined as "8080:8081" instead of
// {"host": "8080", "container": "8081}
impl<'de> Deserialize<'de> for PortMapping {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(serde::de::Error::custom("invalid port mapping"));
        }
        let host = parts[0].parse().map_err(serde::de::Error::custom)?;
        let container = parts[1].parse().map_err(serde::de::Error::custom)?;
        Ok(PortMapping { host, container })
    }
}

// required to have a custom JsonSchema trait implementation to support
// the custom Deserialize trait implementation above.
// PortMapping is represented as a string in the Schema, but deserializes
// to the PortMapping struct
impl JsonSchema for PortMapping {
    fn schema_name() -> String {
        "PortMapping".to_owned()
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let schema = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::String.into()),
            ..Default::default()
        };
        schema.into()
    }
}


#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Metrics {
    pub enabled: bool,
    pub port: String,
    pub path: String,
}


#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Probes {
    pub readiness: Probe,
    pub liveness: Probe,
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Probe {
    pub path: String,
    pub port: String,
    // this should never be negative
    pub initial_delay_seconds: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_port_mapping() {
        let input = r#""8080:8081""#;
        let expected = PortMapping {
            host: 8080,
            container: 8081,
        };
        let actual: PortMapping = serde_json::from_str(input).unwrap();
        assert_eq!(actual, expected);
    }
}
