# tembo-operator

![Version: 0.7.1](https://img.shields.io/badge/Version-0.2.0-informational?style=flat-square) ![Type: application](https://img.shields.io/badge/Type-application-informational?style=flat-square)

Helm chart to deploy the tembo-operator

**Homepage:** <https://tembo.io>

## Maintainers

| Name | Email | Url |
| ---- | ------ | --- |
| Tembo |  | <https://tembocommunity.slack.com> |

## Source Code

* <https://github.com/tembo-io/tembo/tree/main/tembo-operator>
* <https://github.com/cloudnative-pg/cloudnative-pg>

## Requirements

| Repository | Name | Version |
|------------|------|---------|
| https://cloudnative-pg.github.io/charts | cloudnative-pg | 0.20.1  |

## Values

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| cloudnative-pg | object | `{"config":{"create":true,"data":{"INHERITED_ANNOTATIONS":"tembo-pod-init.tembo.io/*, tembo.io/*"}},"enabled":true,"monitoring":{"podMonitorEnabled":false},"service":{"type":"ClusterIP"}}` | Cloudnative-PG configuration |
| cloudnative-pg.config.data.INHERITED_ANNOTATIONS | string | `"tembo-pod-init.tembo.io/*, tembo.io/*"` | INHERITED_ANNOTATIONS needs to match what is set in pod-init namespaceSelector.matchLabels |
| controller | object | `{"affinity":{},"annotations":{},"crds":{"create":true},"enabled":true,"extraEnv":[],"image":{"pullPolicy":"Always","repository":"quay.io/tembo/tembo-operator","tag":"latest"},"livenessProbe":{},"logLevel":"info","monitoring":{"podMonitor":{"enabled":false,"path":"/metrics","port":"http"},"prometheusRule":{"enabled":false}},"nameOverride":null,"namespaceOverride":null,"nodeSelector":{},"podAnnotations":{},"rbac":{"create":true},"readinessProbe":{"httpGet":{"path":"/health","port":"http","scheme":"HTTP"},"initialDelaySeconds":5,"periodSeconds":5},"replicas":1,"resources":{},"service":{"annotations":{},"port":80,"targetPort":8080,"type":"ClusterIP"},"tolerations":[],"upgradeStrategy":"RollingUpdate"}` | The controller configuration |
| controller.affinity | object | `{}` | Affinity for the controller to be installed. |
| controller.annotations | object | `{}` | Annotations to be added to the deployment |
| controller.crds.create | bool | `true` | Specifies whether the CRDs should be created when installing the chart. |
| controller.image | object | `{"pullPolicy":"Always","repository":"quay.io/tembo/tembo-operator","tag":"latest"}` | The default image for the controller |
| controller.image.tag | string | `"latest"` | Overrides the image tag whose default is latest |
| controller.livenessProbe | object | `{}` | LivenessProbe configuration |
| controller.logLevel | string | `"info"` | The log level to set inside the tembo-controller, default is info |
| controller.monitoring.podMonitor | object | `{"enabled":false,"path":"/metrics","port":"http"}` | Specifies whether to enable the PodMonitor.  Requires Prometheus Operator CRDs |
| controller.monitoring.prometheusRule | object | `{"enabled":false}` | Specifies whether the monitoring should be enabled. Requires Prometheus Operator CRDs. |
| controller.nodeSelector | object | `{}` | Nodeselector for the controller to be installed. |
| controller.podAnnotations | object | `{}` | Annotations to be added to the pod. |
| controller.rbac.create | bool | `true` | Specifies whether ClusterRole and ClusterRoleBinding should be created. |
| controller.readinessProbe | object | `{"httpGet":{"path":"/health","port":"http","scheme":"HTTP"},"initialDelaySeconds":5,"periodSeconds":5}` | ReadinessProbe configuration |
| controller.replicas | int | `1` | The number of replicas to set for the tembo-controller |
| controller.service | object | `{"annotations":{},"port":80,"targetPort":8080,"type":"ClusterIP"}` | Service configuraton |
| controller.tolerations | list | `[]` | Tolerations for the controller to be installed. |
| controller.upgradeStrategy | string | `"RollingUpdate"` | Deployment upgradeStrategy configuration |
| pod-init | object | `{"affinity":{},"annotations":{},"enabled":true,"extraEnv":[],"image":{"pullPolicy":"IfNotPresent","repository":"quay.io/tembo/tembo-pod-init","tag":"latest"},"livenessProbe":{"httpGet":{"path":"/health/liveness","port":8443,"scheme":"HTTPS"},"initialDelaySeconds":15},"logLevel":"info","nameOverride":null,"namespaceOverride":null,"namespaceSelector":{"matchLabels":{"tembo-pod-init.tembo.io/watch":"true"}},"nodeSelector":{},"podAnnotations":{},"rbac":{"create":true},"readinessProbe":{"failureThreshold":3,"httpGet":{"path":"/health/readiness","port":8443,"scheme":"HTTPS"},"periodSeconds":15,"timeoutSeconds":15},"replicas":1,"resources":{},"service":{"annotations":{},"port":443,"targetPort":8443,"type":"ClusterIP"},"tolerations":[],"upgradeStrategy":"RollingUpdate"}` | The pod-init configuration |
| pod-init.affinity | object | `{}` | Affinity for the deployment to be installed. |
| pod-init.annotations | object | `{}` | Annotations to be added to the deployment |
| pod-init.image | object | `{"pullPolicy":"IfNotPresent","repository":"quay.io/tembo/tembo-pod-init","tag":"latest"}` | The default image for the pod-init deployment |
| pod-init.image.tag | string | `"latest"` | Overrides the image tag whose default is latest |
| pod-init.livenessProbe | object | `{"httpGet":{"path":"/health/liveness","port":8443,"scheme":"HTTPS"},"initialDelaySeconds":15}` | LivenessProbe configuration |
| pod-init.logLevel | string | `"info"` | The log level to set inside the tembo-controller, default is info |
| pod-init.namespaceSelector | object | `{"matchLabels":{"tembo-pod-init.tembo.io/watch":"true"}}` | Namespace Selector Label confguration |
| pod-init.namespaceSelector.matchLabels | object | `{"tembo-pod-init.tembo.io/watch":"true"}` | Labels to match namespaces for the Mutating Webhook configuation |
| pod-init.nodeSelector | object | `{}` | Nodeselector for the deployment to be installed. |
| pod-init.podAnnotations | object | `{}` | Annotations to be added to the pod |
| pod-init.rbac.create | bool | `true` | Specifies whether ClusterRole and ClusterRoleBinding should be created. |
| pod-init.readinessProbe | object | `{"failureThreshold":3,"httpGet":{"path":"/health/readiness","port":8443,"scheme":"HTTPS"},"periodSeconds":15,"timeoutSeconds":15}` | ReadinessProbe configuration |
| pod-init.replicas | int | `1` | The number of replicas to set for the tembo-controller |
| pod-init.service | object | `{"annotations":{},"port":443,"targetPort":8443,"type":"ClusterIP"}` | Service configuraton |
| pod-init.tolerations | list | `[]` | Tolerations for the deployment to be installed. |
| pod-init.upgradeStrategy | string | `"RollingUpdate"` | Deployment upgradeStrategy configuration |

----------------------------------------------
Autogenerated from chart metadata using [helm-docs v1.11.3](https://github.com/norwoodj/helm-docs/releases/v1.11.3)
