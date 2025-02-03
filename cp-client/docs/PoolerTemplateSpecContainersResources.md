# PoolerTemplateSpecContainersResources

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**claims** | Option<[**Vec<crate::models::PoolerTemplateSpecContainersResourcesClaims>**](PoolerTemplateSpecContainersResourcesClaims.md)> | Claims lists the names of resources, defined in spec.resourceClaims, that are used by this container. This is an alpha field and requires enabling the DynamicResourceAllocation feature gate. This field is immutable. It can only be set for containers. | [optional]
**limits** | Option<[**::std::collections::HashMap<String, crate::models::IntOrString>**](IntOrString.md)> | Limits describes the maximum amount of compute resources allowed. More info: https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/ | [optional]
**requests** | Option<[**::std::collections::HashMap<String, crate::models::IntOrString>**](IntOrString.md)> | Requests describes the minimum amount of compute resources required. If Requests is omitted for a container, it defaults to Limits if that is explicitly specified, otherwise to an implementation-defined value. Requests cannot exceed Limits. More info: https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/ | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


