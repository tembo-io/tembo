/// Types to deserialize Prometheus query responses
pub mod prometheus {
    use serde::de;
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
        pub instance_id: Option<String>,
        pub pod: Option<String>,
        pub namespace: Option<String>,
    }

    fn custom_deserialize_tuple<'de, D>(deserializer: D) -> Result<(i64, i64), D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw_value: (f64, String) = Deserialize::deserialize(deserializer)?;
        let timestamp = raw_value.0.trunc() as i64;
        let parsed_float = raw_value.1.parse::<f64>().map_err(de::Error::custom)?;
        Ok((timestamp, parsed_float as i64))
    }
}

/// Data Plane metrics as packaged to be sent to Control Plane
pub mod dataplane_metrics {
    use super::prometheus::MetricsResult;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct DataPlaneMetrics {
        /// Name of the corresponding metric
        pub name: String,
        /// Results of this metric for all instances
        pub result: Vec<MetricsResult>,
    }

    pub fn split_data_plane_metrics(
        metrics: DataPlaneMetrics,
        max_size: usize,
    ) -> Vec<DataPlaneMetrics> {
        let mut result = Vec::new();
        let mut chunk = Vec::new();

        for item in metrics.result.into_iter() {
            if chunk.len() == max_size {
                result.push(DataPlaneMetrics {
                    name: metrics.name.clone(),
                    result: chunk,
                });
                chunk = Vec::new();
            }
            chunk.push(item);
        }

        if !chunk.is_empty() {
            result.push(DataPlaneMetrics {
                name: metrics.name.clone(),
                result: chunk,
            });
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use crate::metrics::dataplane_metrics::{split_data_plane_metrics, DataPlaneMetrics};
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
              },
              {
                 "metric":{
                    "instance_id":"inst_0000000000001_AAAB0_1",
                    "pod":"org-dummy-2-inst-dummy-1"
                 },
                 "value":[
                    1713365023.028,
                    "1006.123"
                 ]
              }
           ]
        }
     }
    "#;

    const QUERY_REPONSE_LOKI: &str = r#"
    {
      "status": "success",
      "data": {
        "resultType": "vector",
        "result": [
          {
            "metric": {
              "namespace": "api"
            },
            "value": [
              1721419629.355,
              "1"
            ]
          },
          {
            "metric": {
              "namespace": "basically-present-wolfhound"
            },
            "value": [
              1721419629.355,
              "5"
            ]
          },
          {
            "metric": {
              "namespace": "blandly-jovial-limpet"
            },
            "value": [
              1721419629.355,
              "9"
            ]
          },
          {
            "metric": {
              "namespace": "collectively-righteous-doggo"
            },
            "value": [
              1721419629.355,
              "1"
            ]
          },
          {
            "metric": {
              "namespace": "damnably-chunky-peafowl"
            },
            "value": [
              1721419629.355,
              "2"
            ]
          },
          {
            "metric": {
              "namespace": "quickly-chipper-lizard"
            },
            "value": [
              1721419629.355,
              "1"
            ]
          },
          {
            "metric": {
              "namespace": "chipperly-resilient-cat"
            },
            "value": [
              1721419629.355,
              "154"
            ]
          }
        ],
        "stats": {
          "summary": {
            "bytesProcessedPerSecond": 123
          },
          "querier": {
            "store": {
              "totalChunksRef": 0,
              "chunk": {
                "headChunkBytes": 0,
                "headChunkLines": 0
              },
              "chunkRefsFetchTime": 26837494
            }
          },
          "ingester": {
            "totalReached": 32,
            "store": {
              "chunksDownloadTime": 0,
              "chunk": {
                "decompressedStructuredMetadataBytes": 0
              },
              "chunkRefsFetchTime": 0
            }
          }
        }
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
                            instance_id: Some("inst_0000000000000_AAAA0_1".into()),
                            pod: Some("org-dummt-inst-dummy1".into()),
                            namespace: None,
                        },
                        value: (1713365010, 0),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: Some("inst_0000000000001_AAAB0_1".into()),
                            pod: Some("org-dummy-2-inst-dummy-1".into()),
                            namespace: None,
                        },
                        value: (1713365023, 1005),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: Some("inst_0000000000001_AAAB0_1".into()),
                            pod: Some("org-dummy-2-inst-dummy-1".into()),
                            namespace: None,
                        },
                        value: (1713365023, 1006),
                    },
                ],
            },
        };

        assert_eq!(response, expected);
    }

    #[test]
    fn deserializes_loki_responses_correctly() {
        let response: Metrics = serde_json::from_str(QUERY_REPONSE_LOKI).unwrap();

        let expected = Metrics {
            status: "success".into(),
            data: MetricsData {
                result_type: "vector".into(),
                result: vec![
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: None,
                            pod: None,
                            namespace: Some("api".into()),
                        },
                        value: (1721419629, 1),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: None,
                            pod: None,
                            namespace: Some("basically-present-wolfhound".into()),
                        },
                        value: (1721419629, 5),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: None,
                            pod: None,
                            namespace: Some("blandly-jovial-limpet".into()),
                        },
                        value: (1721419629, 9),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: None,
                            pod: None,
                            namespace: Some("collectively-righteous-doggo".into()),
                        },
                        value: (1721419629, 1),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: None,
                            pod: None,
                            namespace: Some("damnably-chunky-peafowl".into()),
                        },
                        value: (1721419629, 2),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: None,
                            pod: None,
                            namespace: Some("quickly-chipper-lizard".into()),
                        },
                        value: (1721419629, 1),
                    },
                    MetricsResult {
                        metric: MetricLabels {
                            instance_id: None,
                            pod: None,
                            namespace: Some("chipperly-resilient-cat".into()),
                        },
                        value: (1721419629, 154),
                    },
                ],
            },
        };

        assert_eq!(response, expected);
    }

    #[test]
    fn test_split_data_plane_metrics() {
        let mut results = Vec::new();

        for i in 0..2008 {
            results.push(MetricsResult {
                metric: MetricLabels {
                    instance_id: Some(format!("inst_{}", i)),
                    pod: Some(format!("pod_{}", i)),
                    namespace: None,
                },
                value: (i as i64, i as i64),
            });
        }

        let data_plane_metrics = DataPlaneMetrics {
            name: "test_metric".into(),
            result: results,
        };

        let split_metrics = split_data_plane_metrics(data_plane_metrics, 1000);

        assert_eq!(
            split_metrics.len(),
            3,
            "Expected 3 chunks, got {}",
            split_metrics.len()
        );
        assert_eq!(
            split_metrics[0].result.len(),
            1000,
            "First chunk size incorrect"
        );
        assert_eq!(
            split_metrics[1].result.len(),
            1000,
            "Second chunk size incorrect"
        );
        assert_eq!(
            split_metrics[2].result.len(),
            8,
            "Third chunk size incorrect"
        );
    }
    #[test]
    fn test_split_data_plane_metrics_exact() {
        let mut results = Vec::new();

        for i in 0..1000 {
            results.push(MetricsResult {
                metric: MetricLabels {
                    instance_id: Some(format!("inst_{}", i)),
                    pod: Some(format!("pod_{}", i)),
                    namespace: None,
                },
                value: (i as i64, i as i64),
            });
        }

        let data_plane_metrics = DataPlaneMetrics {
            name: "test_metric".into(),
            result: results,
        };

        let split_metrics = split_data_plane_metrics(data_plane_metrics, 1000);

        assert_eq!(
            split_metrics.len(),
            1,
            "Expected 1 chunks, got {}",
            split_metrics.len()
        );
        assert_eq!(
            split_metrics[0].result.len(),
            1000,
            "First chunk size incorrect"
        );
    }
}
