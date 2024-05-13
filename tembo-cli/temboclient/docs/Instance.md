# Instance

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**app_services** | Option<[**Vec<crate::models::AppType>**](AppType.md)> |  | [optional]
**connection_info** | Option<[**crate::models::ConnectionInfo**](ConnectionInfo.md)> |  | [optional]
**connection_pooler** | Option<[**crate::models::ConnectionPooler**](ConnectionPooler.md)> |  | [optional]
**cpu** | [**crate::models::Cpu**](Cpu.md) |  |
**created_at** | Option<**String**> |  | [optional]
**environment** | [**crate::models::Environment**](Environment.md) |  |
**extensions** | Option<[**Vec<crate::models::ExtensionStatus>**](ExtensionStatus.md)> |  | [optional]
**extra_domains_rw** | Option<**Vec<String>**> |  | [optional]
**first_recoverability_time** | Option<**String**> |  | [optional]
**instance_id** | **String** |  |
**instance_name** | **String** |  |
**ip_allow_list** | Option<**Vec<String>**> |  | [optional]
**last_updated_at** | Option<**String**> |  | [optional]
**memory** | [**crate::models::Memory**](Memory.md) |  |
**organization_id** | **String** |  |
**namespace** | **String** |  |
**postgres_configs** | Option<[**Vec<crate::models::PgConfig>**](PgConfig.md)> |  | [optional]
**postgres_version** | **i32** | Major Postgres version this instance is using. Currently: 14, 15 or 16 |
**replicas** | **i32** |  |
**runtime_config** | Option<[**Vec<crate::models::PgConfig>**](PgConfig.md)> |  | [optional]
**stack_type** | [**crate::models::StackType**](StackType.md) |  |
**state** | [**crate::models::State**](State.md) |  |
**storage** | [**crate::models::Storage**](Storage.md) |  |
**trunk_installs** | Option<[**Vec<crate::models::TrunkInstallStatus>**](TrunkInstallStatus.md)> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
