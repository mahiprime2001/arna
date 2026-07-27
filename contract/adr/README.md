# Architecture Decision Records

Durable record of *why* the platform is shaped the way it is. Each ADR captures a
decision, the alternatives we weighed, and the consequences we accepted — so future
work argues with the reasoning, not with a vacuum.

## Format

```
# ADR-NNN: Title
Status:   Proposed | Accepted | Superseded by ADR-NNN
Date:     YYYY-MM-DD
Context   — the forces at play, evidence available
Decision  — what we chose
Alternatives Considered — what we rejected and why
Consequences — what this costs us, what it buys, what it forecloses
```

Status changes are edits to the existing file, not new files. Superseded ADRs stay —
deleting them destroys the history that makes them useful.

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [001](0001-product-scope.md) | Product Scope — a Workspace Platform, not a VM or OS | Accepted |
| [002](0002-primary-audience.md) | Primary Audience — Individual Developers | Accepted |
| [003](0003-hardware-ownership-model.md) | Hardware Ownership Model | Accepted |
| [004](0004-workspace-identity-model.md) | Workspace Identity Model | Accepted |
| [005](0005-platform-adapter-architecture.md) | Platform Adapter Architecture | Accepted |
| [006](0006-windows-adapter-rendering-input.md) | Windows Adapter — Rendering & Input Strategy | Accepted |
| [007](0007-specification-before-platform-integration.md) | Product Specification Before Platform Integration | Accepted |
