# \SecretsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_secret**](SecretsApi.md#get_secret) | **GET** /{namespace}/secrets/{secret_name} | Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets/{secret_name}
[**get_secret_names**](SecretsApi.md#get_secret_names) | **GET** /{namespace}/secrets | Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets
[**get_secret_names_v1**](SecretsApi.md#get_secret_names_v1) | **GET** /api/v1/orgs/{org_id}/instances/{instance_id}/secrets | 
[**get_secret_v1**](SecretsApi.md#get_secret_v1) | **GET** /api/v1/orgs/{org_id}/instances/{instance_id}/secrets/{secret_name} | 



## get_secret

> ::std::collections::HashMap<String, String> get_secret(namespace, secret_name)
Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets/{secret_name}

Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets/{secret_name}

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**namespace** | **String** | Instance namespace | [required] |
**secret_name** | **String** | Secret name | [required] |

### Return type

**::std::collections::HashMap<String, String>**

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_secret_names

> Vec<crate::models::AvailableSecret> get_secret_names(namespace)
Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets

Please use /api/v1/orgs/{org_id}/instances/{instance_id}/secrets

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**namespace** | **String** | Instance namespace | [required] |

### Return type

[**Vec<crate::models::AvailableSecret>**](AvailableSecret.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_secret_names_v1

> Vec<crate::models::AvailableSecret> get_secret_names_v1(org_id, instance_id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Tembo Cloud Organization ID | [required] |
**instance_id** | **String** | Tembo Cloud Instance ID | [required] |

### Return type

[**Vec<crate::models::AvailableSecret>**](AvailableSecret.md)

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_secret_v1

> ::std::collections::HashMap<String, String> get_secret_v1(org_id, instance_id, secret_name)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**org_id** | **String** | Tembo Cloud Organization ID | [required] |
**instance_id** | **String** | Tembo Cloud Instance ID | [required] |
**secret_name** | **String** | Secret name | [required] |

### Return type

**::std::collections::HashMap<String, String>**

### Authorization

[jwt_token](../README.md#jwt_token)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

