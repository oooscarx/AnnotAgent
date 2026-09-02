# Rust Expert Model Plugin Product Integration

AnnotAgent exposes installed native expert models under **Settings → Expert Model Plugins**. This
surface is backed by the workspace-local Plugin Registry; it is not a static catalog and it does not
write model weights into Git.

## Human-owned installation flow

1. Select a `.annotplugin` package.
2. Verify its deterministic archive, manifest, target executable and file digests.
3. Review publisher, target platforms, resource limits and requested permissions.
4. Explicitly accept the code license and, when applicable, the weight license.
5. Install the exact package version side by side with existing versions.
6. Add every declared checkpoint component. AnnotAgent copies each file into its owner-only local
   model cache and verifies or records its SHA-256 identity.
7. Run **Test**. The Host starts the installed Rust executable with a cleared environment and
   private directories, authenticates the loopback process, discovers capabilities/contracts and
   runs one typed sample inference.

The model stays visible but unselectable while it is `Installed`, `NeedsWeights`, `Disabled`,
`FailedSmokeTest` or `UnsupportedPlatform`. Only `Ready` models with complete availability evidence
are offered as executable Automation bindings.

## Agent authority

Pipeline Builder receives credential-free `ExpertModelManifest` records for installed versions. It
may inspect capabilities, Artifact contracts, prompt requirements, geometry/score semantics,
runtime requirements, license metadata and truthful availability evidence. It may recommend only a
model whose exact qualified ID is currently Available:

```text
plugin:<plugin-id>@<plugin-version>:<model-id>
```

The Agent cannot upload or install packages, accept terms, provision weights, enable arbitrary
binaries or make an unavailable model Ready. An unresolved expert-model requirement remains an
editable Draft state and links the user back to Plugin setup.

## Publication and execution identity

Publishing freezes the plugin ID and version, deterministic package SHA-256, Plugin API version,
worker protocol version, package-local model ID and revision, checkpoint SHA-256, capability
contract SHA-256 and declared capabilities. The exact installed version is then referenced by the
immutable Workflow Version, so uninstall is rejected until no Published Workflow references it.

A new Run fails closed when the frozen version is missing, disabled, unavailable or has a different
package, contract or checkpoint identity. Runtime launches the exact installed Rust process and
maps the qualified Workflow binding to the package-local model ID only at the authenticated Host
boundary. Core and generic Skills contain no YOLO, SAM, PIDNet, RF-DETR or RoboCup dispatch branch.

## Product surfaces

- GUI: full install, weight, test, enable/disable, reference-aware uninstall and evidence flow.
- CLI: packaging, verification and the complete administrative lifecycle.
- TUI: `/plugins` and `/plugins models` provide read-only operational discovery.
- Workflow editor: compatible Ready models appear by capability; unresolved bindings open Plugin
  setup rather than inventing a fixture.

Package paths, cache paths, process tokens and credentials are never returned to the browser.
