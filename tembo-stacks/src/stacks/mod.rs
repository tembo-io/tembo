pub mod config_engines;
pub mod types;

use crate::stacks::types::{Stack, StackType};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref API: Stack =
        serde_yaml::from_str(include_str!("specs/api.yaml")).expect("api.yaml not found");
    pub static ref DATAWAREHOUSE: Stack =
        serde_yaml::from_str(include_str!("specs/data_warehouse.yaml"))
            .expect("data_warehouse.yaml not found");
    pub static ref GEOSPATIAL: Stack =
        serde_yaml::from_str(include_str!("specs/gis.yaml")).expect("gis.yaml not found");
    pub static ref ML: Stack = serde_yaml::from_str(include_str!("specs/machine_learning.yaml"))
        .expect("machine_learning.yaml not found");
    pub static ref MONGO_ALTERNATIVE: Stack =
        serde_yaml::from_str(include_str!("specs/mongo_alternative.yaml"))
            .expect("mongo_alternative.yaml not found");
    pub static ref MQ: Stack = serde_yaml::from_str(include_str!("specs/message_queue.yaml"))
        .expect("message_queue.yaml not found");
    pub static ref OLAP: Stack =
        serde_yaml::from_str(include_str!("specs/olap.yaml")).expect("olap.yaml not found");
    pub static ref OLTP: Stack =
        serde_yaml::from_str(include_str!("specs/oltp.yaml")).expect("oltp.yaml not found");
    pub static ref RAG: Stack =
        serde_yaml::from_str(include_str!("specs/rag.yaml")).expect("rag.yaml not found");
    pub static ref STANDARD: Stack =
        serde_yaml::from_str(include_str!("specs/standard.yaml")).expect("standard.yaml not found");
    pub static ref TIMESERIES: Stack = serde_yaml::from_str(include_str!("specs/timeseries.yaml"))
        .expect("timeseries.yaml not found");
    pub static ref VECTOR_DB: Stack =
        serde_yaml::from_str(include_str!("specs/vectordb.yaml")).expect("vectordb.yaml not found");
}

pub fn get_stack(entity: StackType) -> Stack {
    match entity {
        StackType::API => API.clone(),
        StackType::DataWarehouse => DATAWAREHOUSE.clone(),
        StackType::Geospatial => GEOSPATIAL.clone(),
        StackType::MachineLearning => ML.clone(),
        StackType::MessageQueue => MQ.clone(),
        StackType::MongoAlternative => MONGO_ALTERNATIVE.clone(),
        StackType::OLAP => OLAP.clone(),
        StackType::OLTP => OLTP.clone(),
        StackType::RAG => RAG.clone(),
        StackType::Standard => STANDARD.clone(),
        StackType::Timeseries => TIMESERIES.clone(),
        StackType::VectorDB => VECTOR_DB.clone(),
    }
}
