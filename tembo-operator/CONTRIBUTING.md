# Contributing to the Tembo Kubernetes Operator
Hello and thank you for your condideration to contribute to the Tembo Kubernetes Operator!

#### Quick notes:
- Interested in the hows and whys of this project? Consider visiting the [README](https://github.com/tembo-io/tembo/blob/main/tembo-operator/README.md)
- Questions or comments? We'd love to hear from you on our [Tembo Slack Channel](https://join.slack.com/t/tembocommunity/shared_invite/zt-277pu7chi-NHtvHWvLhHwyK0Y5Y6vTPw)


## Table of Contents
1. [Prerequisites](#prerequisites) -- minimum software requirement to contribute
2. [Running locally](#running-locally) -- quick guide to geting the operator up and running locally
3. 

## Prerequisites

- [Rust](https://www.rust-lang.org/learn/get-started) - Toolchain including `rustc`, `cargo`, and `rustfmt`
- [Docker Engine](https://docs.docker.com/engine/install/) - For running local containers
- [psql](https://www.postgresql.org/docs/current/app-psql.html) - Terminal-based front-end to PostgreSQL
- [kind](https://github.com/kubernetes-sigs/kind) — simplifies creation of local Kubernetes clusters using Docker (_**K**ubernetes **IN** **D**ocker_)
- [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl) — Kubernetes primary CLI; Docker may include this, but if not be sure to install it
- [helm](https://helm.sh) — the Kubernetes package manager
- [just](https://github.com/casey/just) — simplifies running complex project-specific commands. If you find new useful command, consider adding it to the `justfile`

## Running locally
If you haven't already, clone the tembo repository to your local machine and navigate to the tembo-operator directory.
```
git clone https://github.com/tembo-io/tembo.git
```
```
cd tembo/tembo-operator
```
Once there, run the following to start the Tembo Operator:
```
just start-kind
```
```
just run
```

## Connecting your local docker registry and kind kubernetes cluster


## Enter pod for further testing and exploration


```
kubectl exec -it <your-pod-name> -- /bin/bash
```
