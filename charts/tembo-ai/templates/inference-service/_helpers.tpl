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
Merge configurations with priority to service-specific configs, handling nested structures
*/}}
{{- define "tembo-ai.inferenceService.mergeConfigs" -}}
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
