/// Types to deserialize Prometheus query responses
pub mod prometheus {
    use std::fmt;

    use serde::de;
    use serde::de::SeqAccess;
    use serde::de::Visitor;
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serialize;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct Metrics {
        pub status: String,
        pub data: MetricsData,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct MetricsData {
        pub result_type: String,
        pub result: Vec<MetricsResult>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct MetricsResult {
        // This value does not come in the Prometheus response,
        // we add it in later.
        pub metric: MetricLabels,
        #[serde(deserialize_with = "custom_deserialize_tuple")]
        pub value: (i64, i64),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct MetricLabels {
        pub instance_id: String,
        pub pod: String,
    }

    fn custom_deserialize_tuple<'de, D>(deserializer: D) -> Result<(i64, i64), D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TupleVisitor;

        impl<'de> Visitor<'de> for TupleVisitor {
            type Value = (i64, i64);

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a tuple of (f64, String)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let f64_val: f64 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let str_val: &str = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;

                let timestamp = f64_val.trunc() as i64;
                let parsed_int = str_val
                    .parse::<i64>()
                    .map_err(|_| de::Error::custom("Failed to parse string into integer"))?;

                Ok((timestamp, parsed_int))
            }
        }

        deserializer.deserialize_seq(TupleVisitor)
    }
}

/// Data Plane metrics as packaged to be sent to Control Plane
pub mod dataplane_metrics {
    use serde::{Deserialize, Serialize};

    use super::prometheus::MetricsResult;

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct DataPlaneMetrics {
        /// Name of the corresponding metric
        pub name: String,
        /// Results of this metric for all instances
        pub result: Vec<MetricsResult>,
    }
}

#[cfg(test)]
mod tests {
    use crate::metrics::prometheus::{MetricLabels, Metrics, MetricsData, MetricsResult};

    const QUERY_RESPONSE: &str = r#"
    {
        "status":"success",
        "data":{
           "resultType":"vector",
           "result":[
              {
                 "metric":{
                    "instance_id":"inst_0000000000000_AAAA0_1",
                    "pod":"org-dummt-inst-dummy1"
                 },
                 "value":[
                    1713365010.028,
                    "0"
                 ]
              },
              {
                 "metric":{
                    "instance_id":"inst_0000000000001_AAAB0_1",
                    "pod":"org-dummy-2-inst-dummy-1"
                 },
                 "value":[
                    1713365023.028,
                    "1005"
                 ]
              }
           ]
        }
     }
    "#;

    #[test]
    fn deserializes_prometheus_responses_correctly() {
        let response: Metrics = serde_json::from_str(QUERY_RESPONSE).unwrap();

        let expected = Metrics {
            status: "success".into(),
            data: MetricsData {
                result_type: "vector".into(),
                result: vec![
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: "inst_0000000000000_AAAA0_1".into(),
                            pod: "org-dummt-inst-dummy1".into(),
                        },
                        value: (1713365010, 0),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: "inst_0000000000001_AAAB0_1".into(),
                            pod: "org-dummy-2-inst-dummy-1".into(),
                        },
                        value: (1713365023, 1005),
                    },
                ],
            },
        };

        assert_eq!(response, expected);
    }
}
