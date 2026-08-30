# Provider and Model Registry

AnnotAgent treats connection configuration, selectable models, executable backends, workflow
nodes, Skills, and Agent tools as separate contracts.

- A **Provider Profile** is a reusable service/account connection: adapter protocol, base URL,
  safe metadata, connection policy, health, and an opaque credential reference. It never names the
  model that a Project must use.
- A **Model Profile** is one user-selectable remote model identity on one Provider. It declares
  modalities, protocol features, task capabilities, limits, generation defaults, pricing
  provenance, availability, and a semantic revision.
- A runtime **Model Descriptor** connects a resolved Model Profile to an executable backend. It is
  not the durable product configuration and cannot own credentials.
- A **Skill** contributes domain behavior and registered operations. It does not create Providers,
  store credentials, or choose an unregistered model.
- A **Node** is one typed operation in a Workflow. A node may carry an explicit Model Profile
  binding, but it cannot contain a raw API key or an arbitrary endpoint.
- An **Agent Tool** is a bounded product action over registered objects. It is not a Workflow node
  and cannot read credentials, create Providers, publish, or start a full Dataset Batch.

## Revisions and snapshots

The first Model Profile revision is `1`. Changing the Provider identity, remote model ID,
modalities, protocol features, task capabilities, limits, or generation defaults requires the next
revision. Display-name, availability, lock, pricing, and credential rotation are non-semantic
metadata updates and keep the current revision.

A Published Workflow snapshot freezes the Model Profile ID/revision, Provider adapter and base URL,
remote model ID, modalities, protocol features, task capabilities, limits, and generation defaults.
The snapshot contains neither `CredentialReference` nor pricing. Credentials resolve only when a
call is made; the price actually used belongs to that call's usage record.

## Binding hierarchy

Binding resolution is deterministic:

```text
Workflow Node explicit Model Profile
> Project capability binding
> Project role binding
> Global default
```

Project roles include Pipeline Builder, Primary Inference, Detection, Classification,
Segmentation, Verification, and Fallback. The Pipeline Builder role is the Agent model binding.
Locked bindings can be changed by a user but not by the Agent. If no compatible configured model
exists, resolution returns `unresolved model binding`; it never guesses from a Provider or vendor
name.

Compatibility is fail-closed across Provider enable/health, credential presence, Model Profile
enable/status, input modalities, protocol features, and task capabilities. Provider fallback is a
separate bounded route for infrastructure failures (timeout, rate limit, unavailability, temporary
server failure). Empty detections, low confidence, conflicting evidence, and domain validation stay
inside explicit Workflow Decision branches.
