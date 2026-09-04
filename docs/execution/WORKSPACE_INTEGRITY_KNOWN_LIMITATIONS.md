# Workspace Integrity Known Limitations

This file describes the current product, not the target state.

- Security tokens are process-local by design; after a server restart, the Web client transparently establishes a new local session before its next mutation.
- The current plugin package format can prove archive integrity but has no trusted-publisher signature variant. Consequently, native packages are inspect-only in the Web UI and require the explicit CLI trust flow to install.
- Run ownership is lost in the Web API summary and reconstructed by mutable Project name.
- Project-scoped Runs/Review use global pages with query filters; batch detail and canonical nested ownership routes are incomplete.
- URL state does not yet preserve every Draft/version/image/node/artifact selection.
- Workflow Draft writes are last-write-wins and sample-test history is mutable.
- Results and Debug artifact projections are not yet proven distinct for every workflow.
- Review local edit state and cross-project enforcement are not yet fully covered.
- Some controls are dead, misleading, duplicated, or expose Labs behavior without sufficient qualification.
- Large server/application/frontend modules and unbounded/N+1 summary queries remain.
- The Web production bundle currently warns about a JavaScript chunk above 500 kB.
- Real provider/model checks are environment-dependent and remain outside offline acceptance until explicitly provisioned.
