# Contributing to the Tembo Kubernetes Operator

Welcome!
And thank you for your interest in contributing to the Tembo Kubernetes Operator.
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
3. [Updating the CRD (CustomResourceDefinition)](#updating-the-crd)
   1. [Making and applying changes](#1.-making-and-applying-changes)
4. [Testing](#testing)

## Prerequisites

- [Rust](https://www.rust-lang.org/learn/get-started) - Toolchain including `rustc`, `cargo`, and `rustfmt`
- [Docker Engine](https://docs.docker.com/engine/install/) - For running local containers
- [psql](https://www.postgresql.org/docs/current/app-psql.html) - Terminal-based front-end to PostgreSQL
- [kind](https://github.com/kubernetes-sigs/kind) — Simplifies creation of local Kubernetes clusters using Docker (_**K**ubernetes **IN** **D**ocker_)
- [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl) — Kubernetes primary CLI
- [just](https://github.com/casey/just) — Simplifies running complex, project-specific commands. If you find a new, useful command, consider adding it to the `justfile`
- [helm](https://helm.sh/) - The package manager for Kubernetes

## Running locally

### 1. Initial setup

If you haven't already, go ahead and clone the tembo repository to your local machine and navigate to the `tembo-operator` directory.

```bash
git clone https://github.com/tembo-io/tembo.git
```

```bash
cd tembo/tembo-operator
```

From there, initiate a local Kubernetes cluster:

```bash
just start-kind
```

:bulb: Details on this command, as well as others that invoke `just` can be found within the directory's `justfile`.

:wrench: If you encounter an error, confirm that your Docker engine is running.

Once complete, start the Tembo Operator:

```bash
just run
```

:bulb: This operation will be running continuously, so we advise opening a new terminal workspace.

### 2. Applying YAML files

The `tembo-operator` directory comes complete with a set of sample YAML files, found at `tembo/tembo-operator/yaml`.

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

The next section deals with creating and loading Docker images.
If you'd like to skip it, click the following to learn how to [connect via psql](#4.-connect-via-psql).

### 3. Loading Docker images

In each sample YAML file, notice a specific image being used.
In the case of `sample-standard.yaml` it's `image: "quay.io/tembo/standard-cnpg:15-a0a5ab5"`

The `standard-cnpg` image, as well as the other images hosted at [Tembo's Quay Repository](https://quay.io/organization/tembo), are made using Dockerfiles.
The source material, as well as additional metadata, can be found within the [tembo-images repository](https://github.com/tembo-io/tembo-images).

#### 3.1. Building the image

First start your local Docker registry:

```bash
docker run -d -p 5000:5000 --restart=always --name registry registry:2
```

Then within the same directory as your Dockerfile, run the following:

```bash
docker build -t localhost:5000/my-custom-image:15-0.0.1 .
```

:bulb: Note that not only are the `my-custom-image` name and `15-0.0.1` customizable, but they will crucial in upcomming steps.
Bear this in mind as you define your own names and tags.

#### 3.2. Push to local Docker registry

You can confirm that your local Docker registry is running by invoking `docker ps`.

Then you can push the newly-created image and check for it by running the following:

```bash
docker push localhost:5000/my-custom-image:15-0.0.1
```

```bash
docker images
```

#### 3.3. Apply custom image and connecting

Say, for example you'd like to update the image of the `sample-standard.yaml` file.
You would begin by replacing the image parameter with the following:

```yaml
image: my-custom-image:15-0.0.1
```

Before applying, load the custom image into your kind cluster by running the following:

```bash
kind load docker-image my-custom-image:15-0.0.1
```

Once complete, go ahead any apply the yaml file:

```bash
kubectl apply -f yaml/sample-standard.yaml
```

### 4. Connect via psql

Connecting via psql will require a password, which is linked to your current Kubernetes session.
Sections `4.1` and `4.2` will illustrate how to respectively reveal the password, if you're work is more sessions-based, and how to save the password for later use.

#### 4.1. Revealing password

```bash
kubectl get secrets/sample-standard-connection -o=jsonpath='{.data.password}'
```

The resultant is an encoded password made up of letters and numbers, ending with two equal signs `==`.

Ignore any characters past those, such as a percent symbol `%`.

```bash
echo <your-encoded-secret> | base64 --decode
```

:bulb: The echo statement's output can be used as the password when entering the pod either `psql` or `exec`.

#### 4.2. Saving password

```bash
export PGPASSWORD=$(kubectl get secrets/sample-standard-connection --template={{.data.password}} | base64 -D)
```

Add the following line to /etc/hosts

```
127.0.0.1 sample-standard.localhost
```

```bash
psql postgres://postgres:$PGPASSWORD@sample-standard.localhost:5432
```

### 5. Exec into the pod

Run the following if you are interested in exploring the pod, for example to see where files are saved.

```bash
kubectl exec -it sample-standard-1 -- /bin/bash
```

## Updating the CRD

The Tembo Operator utilizes a Kubernetes CRD (CustomResourceDefinition) with the name `CoreDB`.

If you're not familiar with this topic, please refer to the official [Kubernetes documentation on CRDs](https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/#customresourcedefinitions) to learn more.

Edit the [CoreDBSpec struct](./src/controller.rs) as needed.

Once completed, run the following:

```bash
just generate-crd
```

## Testing

After making any changes, you can run `just fmt` for invoke `cargo +nightly fmt`.
Moreover, it's good practice to create tests for any new feature introduction.

The Tembo Operator `integration_tests.rs` file can be found [here](https://github.com/tembo-io/tembo/blob/main/tembo-operator/tests/integration_tests.rs), but also at the file path `tembo/tembo-operator/tests/integration_tests.rs`

The tests can be run by invoking `just test`.
