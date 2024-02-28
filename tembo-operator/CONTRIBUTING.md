# Contributing to the Tembo Kubernetes Operator

## Prerequisites

- [Rust](https://www.rust-lang.org/learn/get-started) - Toolchain including `rustc`, `cargo`, and `rustfmt`
- [Docker Engine](https://docs.docker.com/engine/install/) - For running local containers
- [psql](https://www.postgresql.org/docs/current/app-psql.html) - Terminal-based front-end to PostgreSQL
- [kind](https://github.com/kubernetes-sigs/kind) — simplifies creation of local Kubernetes clusters using Docker (_**K**ubernetes **IN** **D**ocker_)
- [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl) — Kubernetes primary CLI; Docker may include this, but if not be sure to install it
- [helm](https://helm.sh) — the Kubernetes package manager
- [just](https://github.com/casey/just) — simplifies running complex project-specific commands. If you find new useful command, consider adding it to the `justfile`
- [git]

## Building from source

### Clone repository and navigate to the `tembo-operator` directory
```
git clone https://github.com/tembo-io/tembo.git
```
```
cd tembo/tembo-operator/
```


