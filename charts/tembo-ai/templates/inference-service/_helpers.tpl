{{/*
Inference service specific labels
*/}}
{{- define "tembo-ai.inferenceService.labels" -}}
{{ include "tembo-ai.labels" . }}
{{- end }}

{{/*
Inference service specific selector labels
*/}}
{{- define "tembo-ai.inferenceService.selectorLabels" -}}
app.kubernetes.io/name: {{ include "tembo-ai.name" . }}-service
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the inference-service service account to use
*/}}
{{- define "tembo-ai.inferenceService.serviceAccountName" -}}
{{- include "tembo-ai.fullname" . }}-service
{{- end }}

{{/* 
Define the namespace to use across the inference-service templates
*/}}
{{- define "tembo-ai.namespace" -}}
{{- default .Release.Namespace .Values.namespace }}
{{- end -}}

{{/*
Define the image configuration with override options
*/}}
{{- define "tembo-ai.inferenceService.imageConfig" -}}
{{- $defaultConfig := .defaultConfig -}}
{{- $serviceConfig := .serviceConfig -}}
{{- $result := dict -}}
{{- range $key, $value := $defaultConfig -}}
  {{- if hasKey $serviceConfig $key -}}
    {{- $_ := set $result $key (index $serviceConfig $key) -}}
  {{- else -}}
    {{- $_ := set $result $key $value -}}
  {{- end -}}
{{- end -}}
{{- range $key, $value := $serviceConfig -}}
  {{- if not (hasKey $result $key) -}}
    {{- $_ := set $result $key $value -}}
  {{- end -}}
{{- end -}}
{{- $result | toYaml -}}
{{- end -}}

{{/*
Merge configurations with priority to service-specific configs, handling nested structures
*/}}
{{- define "tembo-ai.mergeConfigs" -}}
{{- $result := dict -}}
{{- range $key, $value := .defaults -}}
  {{- if kindIs "map" $value -}}
    {{- if hasKey $.service $key -}}
      {{- $nested := dict "defaults" $value "service" (index $.service $key) -}}
      {{- $_ := set $result $key (fromYaml (include "tembo-ai.mergeConfigs" $nested)) -}}
    {{- else -}}
      {{- $_ := set $result $key $value -}}
    {{- end -}}
  {{- else -}}
    {{- if not (hasKey $.service $key) -}}
      {{- $_ := set $result $key $value -}}
    {{- else -}}
      {{- $_ := set $result $key (index $.service $key) -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- range $key, $value := .service -}}
  {{- if not (hasKey $result $key) -}}
    {{- $_ := set $result $key $value -}}
  {{- end -}}
{{- end -}}
{{- $result | toYaml -}}
{{- end -}}
