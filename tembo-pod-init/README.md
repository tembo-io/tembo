# tembo-pod-init

This initContainer will then allow us to bootstrap the folder structure needed to add our required mutability,
while leveraging vanilla CloudNativePG.

It's a webhook in the K8s API server that will call a specific URI and route configured in a
MutatingWebhookConfiguration config which will inject an initContainer into a Pod.

## About admission controllers

There are two types of Admission Controllers: `Validating` and `Mutating`.  This is a `Mutating` controller.

Mutating admission controllers take in Kubernetes resource specifications and return an updated resource specification.
They modify the resource attributes before they are passed into subsequent phases. They also perform side-effect
calculations or make external calls (in the case of custom admission controllers).
