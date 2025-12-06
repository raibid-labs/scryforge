# Release Process

This document describes the release process for Scryforge.

## Overview

Scryforge uses semantic versioning and automated releases via GitHub Actions. The release workflow handles:
- Building binaries for multiple platforms
- Publishing crates to crates.io
- Creating GitHub releases with changelogs
- Uploading release artifacts

## Versioning

Scryforge follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** version: Incompatible API changes
- **MINOR** version: Backwards-compatible functionality additions
- **PATCH** version: Backwards-compatible bug fixes

Pre-release versions may use suffixes: `-alpha`, `-beta`, `-rc.1`, etc.

### Version Synchronization

All workspace crates should maintain synchronized versions. When releasing:
1. Update version in workspace `Cargo.toml` (`[workspace.package]` section)
2. Update version in all crate `Cargo.toml` files
3. Verify with `cargo metadata` that all versions match

## Release Types

### Regular Release

A regular release publishes to crates.io and creates a GitHub release with binaries.

**Steps:**
1. Ensure all changes are merged to `main`
2. Update versions in all `Cargo.toml` files
3. Update `CHANGELOG.md` (if maintained separately)
4. Create and push a version tag:
   ```bash
   git tag -a v0.1.0 -m "Release v0.1.0"
   git push origin v0.1.0
   ```
5. GitHub Actions will automatically:
   - Build binaries for all platforms
   - Publish crates to crates.io
   - Create a GitHub release with changelog
   - Upload binary artifacts

### Dry Run Release

Test the release process without publishing:

1. Go to Actions tab in GitHub
2. Select "Release" workflow
3. Click "Run workflow"
4. Check "Dry run" option
5. Review artifacts in workflow run

This builds all binaries and simulates the release process without publishing.

## Release Checklist

Before creating a release tag:

- [ ] All PRs for the release are merged to `main`
- [ ] CI passes on `main` (fmt, clippy, tests)
- [ ] Version numbers updated in all `Cargo.toml` files
- [ ] `CHANGELOG.md` updated (if maintained)
- [ ] Documentation is up-to-date
- [ ] Breaking changes are clearly documented
- [ ] Migration guide created (if needed)
- [ ] Dry run release succeeded

## Crate Publishing Order

Crates are published in dependency order:

1. `fusabi-streams-core` (no internal dependencies)
2. `fusabi-tui-core` (depends on streams-core)
3. `fusabi-tui-widgets` (depends on tui-core)
4. `scryforge-daemon` (depends on streams-core)
5. `scryforge-tui` (depends on all fusabi crates)

The release workflow handles this automatically with appropriate delays.

## Binary Distribution

### Supported Platforms

Binaries are built for:
- Linux (x86_64-unknown-linux-gnu)
- Linux musl (x86_64-unknown-linux-musl)
- macOS Intel (x86_64-apple-darwin)
- macOS ARM (aarch64-apple-darwin)
- Windows (x86_64-pc-windows-msvc)

### Binary Naming

Release artifacts follow this pattern:
```
scryforge-{VERSION}-{TARGET}.{tar.gz|zip}
```

Example: `scryforge-0.1.0-x86_64-unknown-linux-gnu.tar.gz`

### Installation Methods

**From GitHub Release:**
```bash
# Download and extract
wget https://github.com/raibid-labs/scryforge/releases/download/v0.1.0/scryforge-0.1.0-x86_64-unknown-linux-gnu.tar.gz
tar xzf scryforge-0.1.0-x86_64-unknown-linux-gnu.tar.gz

# Move to PATH
sudo mv scryforge-daemon scryforge-tui /usr/local/bin/
```

**From crates.io:**
```bash
cargo install scryforge-daemon
cargo install scryforge-tui
```

**From Homebrew (planned):**
```bash
brew install raibid-labs/tap/scryforge
```

## Hotfix Process

For critical bug fixes on a released version:

1. Create a hotfix branch from the release tag:
   ```bash
   git checkout -b hotfix/v0.1.1 v0.1.0
   ```
2. Apply the fix and update version to patch level
3. Create PR to `main` for the hotfix
4. After merge, tag the hotfix:
   ```bash
   git tag -a v0.1.1 -m "Hotfix v0.1.1"
   git push origin v0.1.1
   ```

## Pre-release Process

For alpha/beta releases:

1. Use pre-release version suffix: `0.2.0-beta.1`
2. Tag with pre-release suffix:
   ```bash
   git tag -a v0.2.0-beta.1 -m "Beta release v0.2.0-beta.1"
   git push origin v0.2.0-beta.1
   ```
3. GitHub will mark the release as "pre-release" automatically

## Rollback Procedure

If a release has critical issues:

1. **Yank from crates.io** (if published):
   ```bash
   cargo yank --vers 0.1.0 scryforge-daemon
   cargo yank --vers 0.1.0 scryforge-tui
   # Yank all affected crates
   ```

2. **Mark GitHub release as draft** or delete if necessary

3. **Create hotfix** following hotfix process above

4. **Communicate** the issue and resolution to users

## Secrets and Credentials

Required GitHub secrets for releases:

- `CARGO_REGISTRY_TOKEN`: Token for publishing to crates.io
  - Generate at https://crates.io/me
  - Add to repository secrets
  - Needs publish permissions for all scryforge crates

## Troubleshooting

### Release workflow fails on crates.io publish

- Check that `CARGO_REGISTRY_TOKEN` is set correctly
- Verify you're an owner of all crates
- Ensure version doesn't already exist on crates.io
- Check that all dependency versions are published

### Binary build fails for a platform

- Check platform-specific dependencies in Cargo.toml
- Review build logs for missing system libraries
- Test locally with `cross` if possible:
  ```bash
  cargo install cross
  cross build --target x86_64-unknown-linux-musl --release
  ```

### Changelog generation is empty

- Ensure you're tagging from `main` with proper commit history
- Verify previous tag exists and is reachable
- Check that commits follow conventional format

## Future Improvements

Planned enhancements to the release process:

- [ ] Automated version bumping via CI
- [ ] Conventional commits enforcement
- [ ] Automated changelog generation
- [ ] Homebrew formula auto-update
- [ ] Docker image publishing
- [ ] Release notes template
- [ ] Post-release smoke tests
