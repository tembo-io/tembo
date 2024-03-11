use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, ToSchema)]
pub enum StackType {
    API,
    DataWarehouse,
    Geospatial,
    MachineLearning,
    MessageQueue,
    MongoAlternative,
    OLAP,
    #[default]
    OLTP,
    RAG,
    Standard,
    Timeseries,
    VectorDB,
}

impl std::str::FromStr for StackType {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "API" => Ok(StackType::API),
            "DataWarehouse" => Ok(StackType::DataWarehouse),
            "Geospatial" => Ok(StackType::Geospatial),
            "MachineLearning" => Ok(StackType::MachineLearning),
            "MessageQueue" => Ok(StackType::MessageQueue),
            "MongoAlternative" => Ok(StackType::MongoAlternative),
            "OLAP" => Ok(StackType::OLAP),
            "OLTP" => Ok(StackType::OLTP),
            "RAG" => Ok(StackType::RAG),
            "Standard" => Ok(StackType::Standard),
            "Timeseries" => Ok(StackType::Timeseries),
            "VectorDB" => Ok(StackType::VectorDB),
            _ => Err("invalid value"),
        }
    }
}

impl StackType {
    pub fn as_str(&self) -> &str {
        match self {
            StackType::API => "API",
            StackType::DataWarehouse => "DataWarehouse",
            StackType::Geospatial => "Geospatial",
            StackType::MachineLearning => "MachineLearning",
            StackType::MessageQueue => "MessageQueue",
            StackType::MongoAlternative => "MongoAlternative",
            StackType::OLAP => "OLAP",
            StackType::OLTP => "OLTP",
            StackType::RAG => "RAG",
            StackType::Standard => "Standard",
            StackType::Timeseries => "Timeseries",
            StackType::VectorDB => "VectorDB",
        }
    }
}

#[cfg(test)]
mod tests {
    use tembo_controller::apis::postgres_parameters::PgConfig;
    use tembo_controller::stacks::{get_stack, types::Infrastructure, types::StackType};

    #[test]
    fn test_stacks_definitions() {
        let mut mq = get_stack(StackType::MessageQueue);
        let infra = Infrastructure {
            cpu: "1".to_string(),
            memory: "1Gi".to_string(),
            storage: "10Gi".to_string(),
        };
        mq.infrastructure = Some(infra);

        // testing the default instance configurations
        let runtime_configs = mq.runtime_config().expect("expected configs");
        // convert to vec to hashmap because order is not guaranteed
        let hm: std::collections::HashMap<String, PgConfig> = runtime_configs
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        let shared_buffers = hm.get("shared_buffers").unwrap();
        assert_eq!(shared_buffers.name, "shared_buffers");
        assert_eq!(shared_buffers.value.to_string(), "614MB");
        let max_connections = hm.get("max_connections").unwrap();
        assert_eq!(max_connections.name, "max_connections");
        assert_eq!(max_connections.value.to_string(), "107");
        assert!(mq.postgres_metrics.is_some());
        assert!(mq.postgres_config.is_some());
        let mq_metrics = mq.postgres_metrics.unwrap();
        assert_eq!(mq_metrics.queries.len(), 1);
        assert!(mq_metrics.queries.contains_key("pgmq"));
        assert!(mq_metrics.queries["pgmq"].master);
        assert_eq!(mq_metrics.queries["pgmq"].metrics.len(), 5);

        let mut std = get_stack(StackType::Standard);
        let infra = Infrastructure {
            cpu: "1".to_string(),
            memory: "2Gi".to_string(),
            storage: "10Gi".to_string(),
        };
        std.infrastructure = Some(infra);
        println!("STD: {:#?}", std);

        let runtime_configs = std.runtime_config().expect("expected configs");
        let hm: std::collections::HashMap<String, PgConfig> = runtime_configs
            .into_iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        let shared_buffers = hm.get("shared_buffers").unwrap();
        assert_eq!(shared_buffers.name, "shared_buffers");
        assert_eq!(shared_buffers.value.to_string(), "512MB");
    }

    #[test]
    fn test_all_stack_deserialization() {
        // must not panic when reading any stack definitions from yaml
        let all_stacks = vec![
            StackType::API,
            StackType::DataWarehouse,
            StackType::Geospatial,
            StackType::MachineLearning,
            StackType::MessageQueue,
            StackType::MongoAlternative,
            StackType::OLAP,
            StackType::OLTP,
            StackType::RAG,
            StackType::Standard,
            StackType::VectorDB,
        ];

        for stack in all_stacks {
            // this is overly verbose, but we want to ensure each Stack can be deserialized from yaml
            // pattern match on the StackType enum, which if a new stack is added, this test will fail until its updated
            // guarantees all StackTypes are tested
            match stack {
                StackType::API => {
                    get_stack(StackType::API);
                }
                StackType::DataWarehouse => {
                    get_stack(StackType::DataWarehouse);
                }
                StackType::Geospatial => {
                    get_stack(StackType::Geospatial);
                }
                StackType::MachineLearning => {
                    get_stack(StackType::MachineLearning);
                }
                StackType::MessageQueue => {
                    get_stack(StackType::MessageQueue);
                }
                StackType::MongoAlternative => {
                    get_stack(StackType::MongoAlternative);
                }
                StackType::OLAP => {
                    get_stack(StackType::OLAP);
                }
                StackType::OLTP => {
                    get_stack(StackType::OLTP);
                }
                StackType::RAG => {
                    get_stack(StackType::RAG);
                }
                StackType::Standard => {
                    get_stack(StackType::Standard);
                }
                StackType::Timeseries => {
                    get_stack(StackType::Timeseries);
                }
                StackType::VectorDB => {
                    get_stack(StackType::VectorDB);
                }
            }
        }
    }
}
