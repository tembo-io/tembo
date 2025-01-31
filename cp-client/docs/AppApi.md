# \AppApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_all_apps**](AppApi.md#get_all_apps) | **GET** /api/v1/apps | Attributes for all apps
[**get_app**](AppApi.md#get_app) | **GET** /api/v1/apps/{type} | Get the attributes of a single App



## get_all_apps

> Vec<serde_json::Value> get_all_apps()
Attributes for all apps

Attributes for all apps 

### Parameters

This endpoint does not need any parameter.

### Return type

[**Vec<serde_json::Value>**](serde_json::Value.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_app

> serde_json::Value get_app(r#type)
Get the attributes of a single App

Get the attributes of a single App 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**r#type** | **String** |  | [required] |

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

