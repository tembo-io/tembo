# Contributing to the Tembo Kubernetes Operator
Welcome! And thank you for your consideration to contribute to the Tembo Kubernetes Operator!
We'll offer the following points up front for orientation:
- Check out the project's [README](https://github.com/tembo-io/tembo/blob/main/tembo-operator/README.md) to learn more about the hows and whys.
- Questions or comments? We'd love to hear from you on our [Tembo Slack Channel](https://join.slack.com/t/tembocommunity/shared_invite/zt-277pu7chi-NHtvHWvLhHwyK0Y5Y6vTPw)

## Table of Contents
1. [Prerequisites](#prerequisites)
2. [Running locally with Kind](#running-locally)
    1. [Initial setup](#1.-initial-setup)
    2. [Applying YAML files](#2.-applying-yaml-files)
    3. [Loading Docker images](#3.-loading-docker-images)
    4. [Connect via psql](#4.-connect-via-psql)
    5. []()
3. CRD

## Prerequisites

- [Rust](https://www.rust-lang.org/learn/get-started) - Toolchain including `rustc`, `cargo`, and `rustfmt`
- [Docker Engine](https://docs.docker.com/engine/install/) - For running local containers
- [psql](https://www.postgresql.org/docs/current/app-psql.html) - Terminal-based front-end to PostgreSQL
- [kind](https://github.com/kubernetes-sigs/kind) — simplifies creation of local Kubernetes clusters using Docker (_**K**ubernetes **IN** **D**ocker_)
- [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl) — Kubernetes primary CLI; Docker may include this, but if not be sure to install it
- [helm](https://helm.sh) — the Kubernetes package manager
- [just](https://github.com/casey/just) — simplifies running complex project-specific commands. If you find new useful command, consider adding it to the `justfile`

## Running locally

### 1. Initial setup

If you haven't already, go ahead and clone the tembo repository to your local machine and navigate to the `tembo-operator` directory.
```
git clone https://github.com/tembo-io/tembo.git
```
```
cd /tembo/tembo-operator
```
Once there, run the following to start the Tembo Operator:
```
just start-kind
```
```
just run
```
This operation will be running continuously, so we advise opening a new workspace in your termainal.

### 2. Applying YAML files

Apply sample yaml, but this can be extended to your own development.

There is a directory containing sample yamls to load, at the path `/tembo/tembo-operator/yaml` and can be loaded from the tembo-operator directory in the following example:

```bash
kubectl apply -f yaml/sample-standard.yaml
```
Confirm by running the following:
```bash
kubectl get pods
```
```text
NAME                READY   STATUS    RESTARTS   AGE
sample-standard-1   1/1     Running   0          14s
```

### 3. Loading Docker images

[README](https://github.com/tembo-io/tembo-images/blob/main/README.md)

### 4. Connect via psql


### 5. kubectl exec into pod

```bash
kubectl exec -it sample-standard-1 -- /bin/bash
```

## Connecting your local docker registry and kind kubernetes cluster



## Enter pod for further testing and exploration



## Metrics with OpenTelemetry (TDB)



## Testing

