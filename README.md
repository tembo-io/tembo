[![conductor-deploy workflow](https://github.com/CoreDB-io/data-plane/actions/workflows/conductor-deploy.yml/badge.svg?branch=main)](https://github.com/CoreDB-io/data-plane/actions/workflows/conductor-deploy.yml)
# Data Plane
The data plane is a kubernetes cluster that serves as the home for Postgres instances. It consists of components responsible for creating, updating and deleting such instances.

Data planes receive orders from the control plane based on user action. 

![Blank diagram](https://user-images.githubusercontent.com/8935584/224689223-d263d812-e021-4ef4-8239-e0397284cab7.svg)


## Conductor

The conductor is responsible for applying or deleting CoreDB custom resource YAML in the data plane kubernetes cluster.
It performs these actions based on messages written to a queue in the control plane. These messages include the CoreDB spec and the action it should take. Once action is taken, the conductor reports the state of the CoreDB instance back to the control plane.


## CoreDB Operator

The [CoreDB operator](https://github.com/CoreDB-io/coredb/tree/main/coredb-operator) is responsible for creating, updating and deleting CoreDB in the data plane cluster. When the conductor applies or deletes a CoreDB custom resource, the operator takes action based on this request.
