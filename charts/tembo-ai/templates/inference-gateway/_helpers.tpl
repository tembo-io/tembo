{{/*
Inference gateway specific labels
*/}}
{{- define "tembo-ai.inferenceGateway.labels" -}}
app.kubernetes.io/component: inference-gateway
{{ include "tembo-ai.labels" . }}
{{- end }}

{{/*
Inference gateway specific selector labels
*/}}
{{- define "tembo-ai.inferenceGateway.selectorLabels" -}}
app.kubernetes.io/name: {{ include "tembo-ai.name" . }}-inference-gateway
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the inference gateway service account to use
*/}}
{{- define "tembo-ai.inferenceGateway.serviceAccountName" -}}
{{- include "tembo-ai.fullname" . }}-inference-gateway
{{- end }}
