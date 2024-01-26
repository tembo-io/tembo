pub mod config_engines;
pub mod types;

use crate::config::Config;

use types::{Stack, StackType};

use lazy_static::lazy_static;
use std::fs;

lazy_static! {
    pub static ref CONFIG: Config = Config::default();
    pub static ref API: Stack = load_stack_template("src/stacks/templates/api.yaml", &CONFIG);
    pub static ref DATAWAREHOUSE: Stack =
        load_stack_template("src/stacks/templates/data_warehouse.yaml", &CONFIG);
    pub static ref MQ: Stack =
        load_stack_template("src/stacks/templates/message_queue.yaml", &CONFIG);
    pub static ref STANDARD: Stack =
        load_stack_template("src/stacks/templates/standard.yaml", &CONFIG);
    pub static ref ML: Stack =
        load_stack_template("src/stacks/templates/machine_learning.yaml", &CONFIG);
    pub static ref OLAP: Stack = load_stack_template("src/stacks/templates/olap.yaml", &CONFIG);
    pub static ref OLTP: Stack = load_stack_template("src/stacks/templates/oltp.yaml", &CONFIG);
    pub static ref VECTOR_DB: Stack =
        load_stack_template("src/stacks/templates/vectordb.yaml", &CONFIG);
    pub static ref GEOSPATIAL: Stack =
        load_stack_template("src/stacks/templates/gis.yaml", &CONFIG);
    pub static ref MONGO_ALTERNATIVE: Stack =
        load_stack_template("src/stacks/templates/mongo_alternative.yaml", &CONFIG);
}

pub fn get_stack(entity: StackType) -> types::Stack {
    match entity {
        StackType::API => API.clone(),
        StackType::DataWarehouse => DATAWAREHOUSE.clone(),
        StackType::MessageQueue => MQ.clone(),
        StackType::Standard => STANDARD.clone(),
        StackType::MachineLearning => ML.clone(),
        StackType::OLAP => OLAP.clone(),
        StackType::OLTP => OLTP.clone(),
        StackType::VectorDB => VECTOR_DB.clone(),
        StackType::Geospatial => GEOSPATIAL.clone(),
        StackType::MongoAlternative => MONGO_ALTERNATIVE.clone(),
    }
}

fn load_stack_template(stack_template: &str, config: &Config) -> types::Stack {
    let template = fs::read_to_string(stack_template)
        .unwrap_or_else(|_| panic!("{} not found", stack_template));
    let rendered_template = template.replace("{{repository}}", &config.stack_image_repository);
    serde_yaml::from_str(&rendered_template)
        .unwrap_or_else(|_| panic!("Error deserializing stack template: {}", stack_template))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_stack_template_default() {
        let config = Config {
            stack_image_repository: "quay.io/tembo".to_string(),
            ..Config::default()
        };
        let stack = load_stack_template("src/stacks/templates/_test.yaml", &config);
        assert_eq!(
            stack.image.as_deref(),
            Some("quay.io/tembo/standard-cnpg:15.3.0-1-839d08e")
        );
    }

    #[test]
    fn test_load_stack_template() {
        let config = Config {
            stack_image_repository: "01234567890.dkr.ecr.us-east-1.amazonaws.com/tembo-io"
                .to_string(),
            ..Config::default()
        };
        let stack = load_stack_template("src/stacks/templates/_test.yaml", &config);
        assert_eq!(
            stack.image.as_deref(),
            Some("01234567890.dkr.ecr.us-east-1.amazonaws.com/tembo-io/standard-cnpg:15.3.0-1-839d08e")
        );
    }
}
