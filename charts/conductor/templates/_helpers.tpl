{{/*
Expand the name of the chart.
*/}}
{{- define "conductor.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "conductor.fullname" -}}
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
Create chart name and version as used by the chart label.
*/}}
{{- define "conductor.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "conductor.labels" -}}
helm.sh/chart: {{ include "conductor.chart" . }}
{{ include "conductor.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "conductor.selectorLabels" -}}
app.kubernetes.io/name: {{ include "conductor.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "conductor.watcherSelectorLabels" -}}
app.kubernetes.io/name: {{ include "conductor.name" . }}-watcher
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "conductor.podLabels" -}}
{{- if .Values.podLabels }}
{{ toYaml .Values.podLabels }}
{{- end }}
{{- end }}

{{- define "conductor.watcherLabels" -}}
helm.sh/chart: {{ include "conductor.chart" . }}
{{ include "conductor.watcherSelectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "conductor.metricsSelectorLabels" -}}
app.kubernetes.io/name: {{ include "conductor.name" . }}-metrics
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "conductor.metricsLabels" -}}
helm.sh/chart: {{ include "conductor.chart" . }}
{{ include "conductor.metricsSelectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}


{{/*
Create the name of the service account to use
*/}}
{{- define "conductor.serviceAccountName" -}}
{{- default (include "conductor.fullname" .) .Values.serviceAccount.name }}
{{- end }}
