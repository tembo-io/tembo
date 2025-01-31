# \InstanceApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_instance**](InstanceApi.md#create_instance) | **POST** /api/v1/orgs/{org_id}/instances | Create a new Tembo instance
[**delete_instance**](InstanceApi.md#delete_instance) | **DELETE** /api/v1/orgs/{org_id}/instances/{instance_id} | Delete an existing Tembo instance
[**get_all**](InstanceApi.md#get_all) | **GET** /api/v1/orgs/{org_id}/instances | Get all Tembo instances in an organization
[**get_instance**](InstanceApi.md#get_instance) | **GET** /api/v1/orgs/{org_id}/instances/{instance_id} | Get an existing Tembo instance
[**get_schema**](InstanceApi.md#get_schema) | **GET** /api/v1/orgs/instances/schema | Get the json-schema for an instance
[**instance_event**](InstanceApi.md#instance_event) | **POST** /api/v1/orgs/{org_id}/instances/{instance_id} | Lifecycle events for a Tembo instance
[**patch_instance**](InstanceApi.md#patch_instance) | **PATCH** /api/v1/orgs/{org_id}/instances/{instance_id} | Update attributes on an existing Tembo instance
[**put_instance**](InstanceApi.md#put_instance) | **PUT** /api/v1/orgs/{org_id}/instances/{instance_id} | Replace all attributes of an existing Tembo instance
[**restore_instance**](InstanceApi.md#restore_instance) | **POST** /api/v1/orgs/{org_id}/restore | Restore a Tembo instance



## create_instance

> crate::models::Instance create_instance(org_id, create_instance)
Create a new Tembo instance

Create a new Tembo instance 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Organization ID that owns the Tembo instance | [required] |
**create_instance** | [**CreateInstance**](CreateInstance.md) |  | [required] |

### Return type

[**crate::models::Instance**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json, text/plain

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_instance

> crate::models::Instance delete_instance(org_id, instance_id)
Delete an existing Tembo instance

Delete an existing Tembo instance 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Organization id of the instance to delete | [required] |
**instance_id** | **String** | Delete this instance id | [required] |

### Return type

[**crate::models::Instance**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_all

> Vec<crate::models::Instance> get_all(org_id)
Get all Tembo instances in an organization

Get all Tembo instances in an organization 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | organization id for the request | [required] |

### Return type

[**Vec<crate::models::Instance>**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_instance

> crate::models::Instance get_instance(org_id, instance_id)
Get an existing Tembo instance

Get an existing Tembo instance 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Organization ID that owns the instance | [required] |
**instance_id** | **String** |  | [required] |

### Return type

[**crate::models::Instance**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_schema

> crate::models::ErrorResponseSchema get_schema()
Get the json-schema for an instance

Get the json-schema for an instance 

### Parameters

This endpoint does not need any parameter.

### Return type

[**crate::models::ErrorResponseSchema**](ErrorResponseSchema.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## instance_event

> crate::models::Instance instance_event(org_id, event_type, instance_id)
Lifecycle events for a Tembo instance

Lifecycle events for a Tembo instance  Currently only supports restart

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Organization ID that owns the Tembo instance | [required] |
**event_type** | [**InstanceEvent**](.md) |  | [required] |
**instance_id** | **String** |  | [required] |

### Return type

[**crate::models::Instance**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## patch_instance

> crate::models::Instance patch_instance(org_id, instance_id, patch_instance)
Update attributes on an existing Tembo instance

Update attributes on an existing Tembo instance 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Organization ID that owns the instance | [required] |
**instance_id** | **String** |  | [required] |
**patch_instance** | [**PatchInstance**](PatchInstance.md) |  | [required] |

### Return type

[**crate::models::Instance**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json, text/plain

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## put_instance

> crate::models::Instance put_instance(org_id, instance_id, update_instance)
Replace all attributes of an existing Tembo instance

Replace all attributes of an existing Tembo instance 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Organization ID that owns the Tembo instance | [required] |
**instance_id** | **String** |  | [required] |
**update_instance** | [**UpdateInstance**](UpdateInstance.md) |  | [required] |

### Return type

[**crate::models::Instance**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## restore_instance

> crate::models::Instance restore_instance(org_id, restore_instance)
Restore a Tembo instance

Restore a Tembo instance 

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Organization ID that owns the Tembo instance | [required] |
**restore_instance** | [**RestoreInstance**](RestoreInstance.md) |  | [required] |

### Return type

[**crate::models::Instance**](Instance.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json, text/plain

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

