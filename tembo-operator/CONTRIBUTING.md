# Contributing to the Tembo Kubernetes Operator
Welcome!
And thank you for your consideration to contribute to the Tembo Kubernetes Operator.
Here are some quick pointers for orientation:
- Check out the project's [README](https://github.com/tembo-io/tembo/blob/main/tembo-operator/README.md) to learn about the less technical aspects.
- Questions or comments? We'd love to hear from you on our [Tembo Slack Channel](https://join.slack.com/t/tembocommunity/shared_invite/zt-277pu7chi-NHtvHWvLhHwyK0Y5Y6vTPw)!

## Table of Contents
1. [Prerequisites](#prerequisites)
2. [Running locally with Kind](#running-locally)
    1. [Initial setup](#1.-initial-setup)
    2. [Applying YAML files](#2.-applying-YAML-files)
    3. [Loading Docker images](#3.-loading-docker-images)
    4. [Connect via psql](#4.-connect-via-psql)
    5. [Exec into the pod](#5.-exec-into-the-pod)

## Prerequisites

- [Rust](https://www.rust-lang.org/learn/get-started) - Toolchain including `rustc`, `cargo`, and `rustfmt`
- [Docker Engine](https://docs.docker.com/engine/install/) - For running local containers
- [psql](https://www.postgresql.org/docs/current/app-psql.html) - Terminal-based front-end to PostgreSQL
- [kind](https://github.com/kubernetes-sigs/kind) — simplifies creation of local Kubernetes clusters using Docker (_**K**ubernetes **IN** **D**ocker_)
- [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl) — Kubernetes primary CLI; Docker may include this, but if not be sure to install it
- [just](https://github.com/casey/just) — simplifies running complex project-specific commands. If you find new useful command, consider adding it to the `justfile`

## Running locally

### 1. Initial setup

If you haven't already, go ahead and clone the tembo repository to your local machine and navigate to the `tembo-operator` directory.


```bash
git clone https://github.com/tembo-io/tembo.git
```
```bash
cd /tembo/tembo-operator
```
From there, run the following to initiate a local Kubernetes cluster:
```bash
just start-kind
```
Once complete, you can execute the following to start the Tembo Operator:
```bash
just run
```
:bulb: This operation will be running continuously, so we advise opening a new terminal workspace.

### 2. Applying YAML files

The `tembo-operator directory comes complete with a set of sample YAML files, found at `/tembo/tembo-operator/yaml`.

You can try out any of the sample YAML files, for example by running the following:

```bash
kubectl apply -f yaml/sample-standard.yaml
```
After some moments, confirm the newly-made kubernetes pod:
```bash
kubectl get pods
```
```text
NAME                READY   STATUS    RESTARTS   AGE
sample-standard-1   1/1     Running   0          14s
```

### 3. Loading Docker images

Within the sample YAML files, you will notice a specific image being used.
In the case of `sample-standard.yaml` it's `image: "quay.io/tembo/standard-cnpg:15-a0a5ab5"`

You may desire to create a in addition to the images at [Tembo's Quay Repository](https://quay.io/organization/tembo).

#### 3.1. Building the image

```bash
docker build -t localhost:5000/my-custom-image:15-0.0.1 .
```

#### 3.2. Push to local docker registry

```bash
docker push localhost:5000/my-custom-image:15-0.0.1
```

```bash
docker images
```

#### 3.3. Apply custom image and connecting 

```bash
kind load docker-image my-custom-image:15-0.0.1
```

```bash
kubectl apply -f yaml/sample-standard.yaml
```

```bash
kind load docker-image my-custom-image:15-0.0.1
```

### 4. Connect via psql

#### 4.1. Revealing password

```bash
kubectl get secrets/sample-standard-connection -o=jsonpath='{.data.password}'
```
```bash
echo <your-encoded-secret> | base64 --decode
```

#### 4.2. Saving password

```bash
export PGPASSWORD=$(kubectl get secrets/sample-coredb-connection --template={{.data.password}} | base64 -D)
```

Add the following line to /etc/hosts
```
127.0.0.1 sample-coredb.localhost
```

```bash
psql postgres://postgres:$PGPASSWORD@sample-coredb.localhost:5432
```


### 5. Exec into the pod

```bash
kubectl exec -it sample-standard-1 -- /bin/bash
```

## Testing

:bulb: Note that the integration tests assume you already have installed or are running the operator connected to the cluster.

