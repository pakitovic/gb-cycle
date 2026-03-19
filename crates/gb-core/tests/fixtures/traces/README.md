# Trace fixtures

Store small golden trace artifacts here once the debugger and scheduler expose
a stable trace format.

Prefer text formats during the early phases unless binary structure is required.

Keep generic machine and scheduler golden traces at this top level. Phase-scoped
ROM-oracle targets should live under matching subdirectories such as `phase2/`
and `phase4/`, with README stubs when the typed suite contract exists but the
assets are still pending.
