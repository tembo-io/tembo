/*
 * Tembo Cloud
 *
 * Platform API for Tembo Cloud             </br>             </br>             To find a Tembo Data API, please find it here:             </br>             </br>             [AWS US East 1](https://api.data-1.use1.tembo.io/swagger-ui/)
 *
 * The version of the OpenAPI document: v1.0.0
 *
 * Generated by: https://openapi-generator.tech
 */

///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum StackType {
    #[serde(rename = "Standard")]
    Standard,
    #[serde(rename = "MessageQueue")]
    MessageQueue,
    #[serde(rename = "MachineLearning")]
    MachineLearning,
    #[serde(rename = "OLAP")]
    Olap,
    #[serde(rename = "OLTP")]
    Oltp,
    #[serde(rename = "Analytics")]
    Analytics,
    #[serde(rename = "VectorDB")]
    VectorDb,
    #[serde(rename = "DataWarehouse")]
    DataWarehouse,
    #[serde(rename = "Geospatial")]
    Geospatial,
    #[serde(rename = "MongoAlternative")]
    MongoAlternative,
    #[serde(rename = "RAG")]
    Rag,
    #[serde(rename = "Timeseries")]
    Timeseries,
    #[serde(rename = "API")]
    API,
    #[serde(rename = "ParadeDB")]
    ParadeDB,
}

impl ToString for StackType {
    fn to_string(&self) -> String {
        match self {
            Self::Standard => String::from("Standard"),
            Self::MessageQueue => String::from("MessageQueue"),
            Self::MachineLearning => String::from("MachineLearning"),
            Self::Olap => String::from("OLAP"),
            Self::Oltp => String::from("OLTP"),
            Self::Analytics => String::from("Analytics"),
            Self::VectorDb => String::from("VectorDB"),
            Self::DataWarehouse => String::from("DataWarehouse"),
            Self::Geospatial => String::from("Geospatial"),
            Self::MongoAlternative => String::from("MongoAlternative"),
            Self::Rag => String::from("RAG"),
            Self::Timeseries => String::from("Timeseries"),
            Self::API => String::from("API"),
            Self::ParadeDB => String::from("ParadeDB"),
        }
    }
}

impl Default for StackType {
    fn default() -> StackType {
        Self::Standard
    }
}
