# Documentation Structure

This document describes the organization of Scryforge's documentation.

## Directory Layout

```
docs/
├── STRUCTURE.md           # This file - documentation organization guide
├── ARCHITECTURE.md        # Technical architecture and design decisions
├── ROADMAP.md            # Development roadmap and phases
├── PROVIDERS.md          # Provider capability model and implementation guide
├── RELEASE.md            # Release process and versioning guide
└── versions/
    └── vNEXT/            # Documentation for unreleased features
        └── (feature docs go here)
```

## Documentation Types

### Core Documentation (docs/)

These files live at the root of `docs/` and represent the current stable state:

- **STRUCTURE.md**: This file - explains how documentation is organized
- **ARCHITECTURE.md**: System architecture, component responsibilities, API specifications
- **ROADMAP.md**: Development phases and long-term planning
- **PROVIDERS.md**: Provider capability traits and implementation patterns
- **RELEASE.md**: Release process, versioning strategy, and deployment procedures

### Versioned Documentation (docs/versions/)

Version-specific documentation lives under `docs/versions/`:

- **vNEXT/**: Documentation for features under development
  - Place new feature docs here during development
  - Move to root docs/ when the feature is released and stable

Future versioned directories may include:
- **v0.1/**: Historical documentation for v0.1 release
- **v0.2/**: Historical documentation for v0.2 release

## Documentation Guidelines

### When to Create New Docs

- **New features**: Create docs in `docs/versions/vNEXT/` during development
- **Architecture changes**: Update `ARCHITECTURE.md` with explanation and rationale
- **New providers**: Document in `PROVIDERS.md` with capability mapping
- **API changes**: Update relevant section in `ARCHITECTURE.md`

### When to Update Existing Docs

- **Released features**: Move from `vNEXT/` to root when feature ships
- **Bug fixes**: Update affected docs inline
- **Clarifications**: Update immediately (no versioning needed)
- **Deprecations**: Mark deprecated sections clearly with version info

### Writing Style

- Use clear, concise language
- Include code examples where appropriate
- Link to related documentation sections
- Keep examples up-to-date with codebase
- Use ASCII diagrams for architecture illustrations

## Maintenance

### Before Each Release

1. Review `docs/versions/vNEXT/` for completed features
2. Integrate or reference vNEXT docs in main documentation
3. Archive previous version docs if needed
4. Update version references throughout documentation
5. Verify all code examples compile and run

### Avoiding Documentation Debt

- Update docs in the same PR as code changes
- Review docs during code review
- Run doc checks in CI (see `.github/workflows/ci.yml`)
- Keep a "docs TODO" list in vNEXT for incomplete documentation
- Delete outdated or superseded documentation promptly

## CI Integration

Documentation is checked in CI:
- Markdown linting (formatting, broken links)
- Code example validation
- Spell checking

See `.github/workflows/ci.yml` for current checks.
