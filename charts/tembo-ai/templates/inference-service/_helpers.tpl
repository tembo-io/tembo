{{/*
Inference service specific labels
*/}}
{{- define "tembo-ai.inferenceService.labels" -}}
app.kubernetes.io/component: inference-service
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
{{- default .Release.Namespace -}}
{{- end -}}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "tembo-ai.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}


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
