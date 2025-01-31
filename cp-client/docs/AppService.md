# AppService

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**args** | Option<**Vec<String>**> | Defines the arguments to pass into the container if needed. You define this in the same manner as you would for all Kubernetes containers. See the [Kubernetes docs](https://kubernetes.io/docs/tasks/inject-data-application/define-command-argument-container). | [optional]
**command** | Option<**Vec<String>**> | Defines the command into the container if needed. You define this in the same manner as you would for all Kubernetes containers. See the [Kubernetes docs](https://kubernetes.io/docs/tasks/inject-data-application/define-command-argument-container). | [optional]
**env** | Option<[**Vec<crate::models::EnvVar>**](EnvVar.md)> | Defines the environment variables to pass into the container if needed. You define this in the same manner as you would for all Kubernetes containers. See the [Kubernetes docs](https://kubernetes.io/docs/tasks/inject-data-application/define-environment-variable-container). | [optional]
**image** | **String** | Defines the container image to use for the appService. | 
**middlewares** | Option<[**Vec<crate::models::Middleware>**](Middleware.md)> | Defines the ingress middeware configuration for the appService. This is specifically configured for the ingress controller Traefik. | [optional]
**name** | **String** | Defines the name of the appService. | 
**probes** | Option<[**crate::models::Probes**](Probes.md)> |  | [optional]
**resources** | Option<[**crate::models::ResourceRequirements**](ResourceRequirements.md)> |  | [optional]
**routing** | Option<[**Vec<crate::models::Routing>**](Routing.md)> | Defines the routing configuration for the appService. | [optional]
**storage** | Option<[**crate::models::StorageConfig**](StorageConfig.md)> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


