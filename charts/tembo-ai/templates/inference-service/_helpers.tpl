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
