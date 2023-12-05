# UpdateInstance

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**app_services** | Option<[**Vec<crate::models::AppType>**](AppType.md)> |  | [optional]
**connection_pooler** | Option<[**crate::models::ConnectionPooler**](ConnectionPooler.md)> |  | [optional]
**cpu** | [**crate::models::Cpu**](Cpu.md) |  | 
**environment** | [**crate::models::Environment**](Environment.md) |  | 
**extensions** | Option<[**Vec<crate::models::Extension>**](Extension.md)> |  | [optional]
**extra_domains_rw** | Option<**Vec<String>**> |  | [optional]
**ip_allow_list** | Option<**Vec<String>**> |  | [optional]
**memory** | [**crate::models::Memory**](Memory.md) |  | 
**postgres_configs** | Option<[**Vec<crate::models::PgConfig>**](PgConfig.md)> |  | [optional]
**replicas** | **i32** |  | 
**storage** | [**crate::models::Storage**](Storage.md) |  | 
**trunk_installs** | Option<[**Vec<crate::models::TrunkInstall>**](TrunkInstall.md)> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


