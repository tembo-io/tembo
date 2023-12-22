# \MetricsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**query_range**](MetricsApi.md#query_range) | **GET** /{namespace}/metrics/query_range | 



## query_range

> serde_json::Value query_range(namespace, query, start, end, step)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**namespace** | **String** | Instance namespace | [required] |
**query** | **String** | PromQL range query, must include a 'namespace' label matching the query path | [required] |
**start** | **i64** | Range start, unix timestamp | [required] |
**end** | Option<**i64**> | Range end, unix timestamp. Default is now. |  |
**step** | Option<**String**> | Step size duration string, defaults to 60s |  |

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

