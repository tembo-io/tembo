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

## Local development

Start a cluster, this will also start the operator.

```
# start cluster
just start-kind
```

For the webhook to work, you will need to setup a forward and the best way to do this is by using [ngrok](https://ngrok.com/).

This will allow the Kubernetes API server to forward webhook requests to your service which will be running locally.

```
# Run your ngrok server
> ngrok http https://localhost:8443
```

Please make node of the URL once you are connected.  Looking at the example below we will be using the `Forwarding` address `https://b224-99-108-68-110.ngrok-free.app`

```
Session Status                online
Account                       nhudson (Plan: Free)
Version                       3.20.0
Region                        United States (us)
Latency                       25ms
Web Interface                 http://127.0.0.1:4040
Forwarding                    https://b224-99-108-68-110.ngrok-free.app -> https://localhost:8443
```

Setup some local TLS certificates

```
> scripts/gen-certificates.sh
```

Generate the `MutatingWebhookConfiguration`, you will need the forwarding address from ngrok above

```
just install-webhook <forwarding URL>
```

Run the pod-init service locally

```
just watch
```

