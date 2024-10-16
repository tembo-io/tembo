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
Deepmerge the inference-service default configs and the services sepecific configs
*/}}
{{- define "tembo-ai.inferenceService.deepMerge" -}}
{{- $result := deepCopy (index . 0) -}}
{{- range $key, $value := index . 1 -}}
  {{- if kindIs "map" $value -}}
    {{- if hasKey $result $key -}}
      {{- $newValue := fromYaml (include "tembo-ai.inferenceService.deepMerge" (list (get $result $key) $value)) -}}
      {{- $_ := set $result $key $newValue -}}
    {{- else -}}
      {{- $_ := set $result $key $value -}}
    {{- end -}}
  {{- else -}}
    {{- $_ := set $result $key $value -}}
  {{- end -}}
{{- end -}}
{{- $result | toYaml -}}
{{- end -}}
