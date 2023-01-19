[![reconciler-deploy workflow](https://github.com/CoreDB-io/data-plane/actions/workflows/reconciler-deploy.yml/badge.svg?branch=main)](https://github.com/CoreDB-io/data-plane/actions/workflows/reconciler-deploy.yml)
# Data Plane (POC)

The data plane (tentatively) consists of the following components:

1. [Reconciler](https://github.com/CoreDB-io/data-plane/tree/main/reconciler)

## Reconciler - `./reconciler/`

The reconciler is responsible for creating, updating, deleting database instances (custom resource) on a kubernetes cluster.
It runs in each data plane and performs these actions based on messages written to a queue in the control plane.
