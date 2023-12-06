# \StackApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_all_entities**](StackApi.md#get_all_entities) | **GET** /api/v1/stacks | Attributes for all stacks
[**get_entity**](StackApi.md#get_entity) | **GET** /api/v1/stacks/{type} | Get the attributes of a single stack



## get_all_entities

> Vec<serde_json::Value> get_all_entities()
Attributes for all stacks

Attributes for all stacks 

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


## get_entity

> serde_json::Value get_entity(r#type)
Get the attributes of a single stack

Get the attributes of a single stack 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**r#type** | [**StackType**](.md) | the type of entity | [required] |

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

