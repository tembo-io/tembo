pub mod config_engines;
pub mod types;

use types::{Stack, StackType};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref DATAWAREHOUSE: Stack = serde_yaml::from_str(include_str!("templates/data_warehouse.yaml"))
        .expect("data_warehouse.yaml not found");
    pub static ref MQ: Stack = serde_yaml::from_str(include_str!("templates/message_queue.yaml"))
        .expect("message_queue.yaml not found");
    pub static ref STANDARD: Stack =
        serde_yaml::from_str(include_str!("templates/standard.yaml")).expect("standard.yaml not found");
    pub static ref ML: Stack = serde_yaml::from_str(include_str!("templates/machine_learning.yaml"))
        .expect("machine_learning.yaml not found");
    pub static ref OLAP: Stack =
        serde_yaml::from_str(include_str!("templates/olap.yaml")).expect("olap.yaml not found");
    pub static ref OLTP: Stack =
        serde_yaml::from_str(include_str!("templates/oltp.yaml")).expect("oltp.yaml not found");
    pub static ref VECTOR_DB: Stack =
        serde_yaml::from_str(include_str!("templates/vectordb.yaml")).expect("vectordb.yaml not found");
    pub static ref GEOSPATIAL: Stack =
        serde_yaml::from_str(include_str!("templates/gis.yaml")).expect("gis.yaml not found");
    pub static ref MONGO_ADAPTER: Stack =
        serde_yaml::from_str(include_str!("templates/document.yaml")).expect("document.yaml not found");
}

pub fn get_stack(entity: StackType) -> types::Stack {
    match entity {
        StackType::DataWarehouse => DATAWAREHOUSE.clone(),
        StackType::MessageQueue => MQ.clone(),
        StackType::Standard => STANDARD.clone(),
        StackType::MachineLearning => ML.clone(),
        StackType::OLAP => OLAP.clone(),
        StackType::OLTP => OLTP.clone(),
        StackType::VectorDB => VECTOR_DB.clone(),
        StackType::Geospatial => GEOSPATIAL.clone(),
        StackType::Document => DOCUMENT.clone(),
    }
}
