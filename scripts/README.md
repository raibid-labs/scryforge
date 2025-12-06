# Scripts

This directory contains Nushell scripts for development workflows and policy enforcement.

## Requirements

- [Nushell](https://www.nushell.sh/) (v0.80+)
- Rust toolchain with `rustfmt` and `clippy` components

## Available Scripts

### CI Checks

- **`check-fmt.nu`**: Verify Rust code formatting
  ```bash
  nu scripts/check-fmt.nu
  ```

- **`check-clippy.nu`**: Run Clippy lints with warnings as errors
  ```bash
  nu scripts/check-clippy.nu
  ```

- **`run-tests.nu`**: Execute the full test suite
  ```bash
  nu scripts/run-tests.nu
  ```

- **`ci-local.nu`**: Run all CI checks locally before pushing
  ```bash
  nu scripts/ci-local.nu
  ```

## Usage

### Before Committing

Run the full CI suite locally:
```bash
nu scripts/ci-local.nu
```

This runs the same checks that GitHub Actions will run on your PR.

### Individual Checks

Run individual checks for faster feedback:

```bash
# Just format check
nu scripts/check-fmt.nu

# Just Clippy
nu scripts/check-clippy.nu

# Just tests
nu scripts/run-tests.nu
```

## Policy Adoption

These scripts enforce the project's code quality policies:

1. **Formatting**: All code must be formatted with `cargo fmt`
2. **Linting**: All code must pass `cargo clippy` with no warnings
3. **Testing**: All tests must pass before merging

The same checks run in CI (`.github/workflows/ci.yml`), so running them locally helps catch issues early.

## Adding New Scripts

When adding new scripts:

1. Use Nushell for consistency
2. Add executable permissions: `chmod +x scripts/your-script.nu`
3. Include usage documentation in script comments
4. Update this README with script description
5. Consider adding to `ci-local.nu` if it's a CI check
