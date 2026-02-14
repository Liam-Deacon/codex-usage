# AGENTS.md — Release & CI/CD Guide

## Version Management

- **Source of truth**: Git tags (e.g., `v0.1.5`)
- **Cargo.toml** contains a base version, but CI patches it from the tag at build time
- **pyproject.toml** uses `dynamic = ["version"]` — maturin reads version from Cargo.toml
- **Cargo.lock** must be committed (required for reproducible builds and Docker)

## Release Process

```
git tag v0.1.5
git push origin v0.1.5
```

This triggers the following chain:

1. **ci.yaml** (on tag push `v*`):
   - Builds release binaries for 5 platform targets
   - Packages as `.tar.gz` (Unix) / `.zip` (Windows)
   - Creates a **published** GitHub Release with all artifacts
   - Calls downstream workflows via `workflow_call` (see note below)

2. **docker.yml** (called by ci.yaml after publish):
   - Builds multi-arch Docker image (`linux/amd64`, `linux/arm64`)
   - Pushes to `ghcr.io/<owner>/codex-usage` with semver tags

3. **pypi.yaml** (called by ci.yaml after publish):
   - Builds wheels for all platforms + sdist
   - Publishes to TestPyPI, then PyPI (using trusted publishing)

> **Note**: Docker and PyPI workflows are triggered via `workflow_call` from ci.yaml
> rather than `release: published` events, because `GITHUB_TOKEN`-created events
> don't trigger other workflows (GitHub Actions limitation). Both workflows also
> support `workflow_dispatch` for manual runs.

## Platform Matrix

| Platform | Target | Runner |
|----------|--------|--------|
| macOS x64 | `x86_64-apple-darwin` | `macos-14` |
| macOS ARM64 | `aarch64-apple-darwin` | `macos-latest` |
| Linux x64 | `x86_64-unknown-linux-gnu` | `ubuntu-latest` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `ubuntu-latest` (cross) |
| Windows x64 | `x86_64-pc-windows-msvc` | `windows-latest` |

## Docker Images

- Registry: `ghcr.io/<owner>/codex-usage`
- Tags: `latest` (main branch), `v0.1.5`, `v0.1`, `v0`, SHA
- Architectures: `linux/amd64`, `linux/arm64`

## PyPI Package

- Package name: `codex-usage`
- Built with maturin (Rust + PyO3 bindings)
- Wheel targets: `x86_64-manylinux`, `aarch64-manylinux`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`

## Important Notes

- Never create GitHub Releases as drafts — downstream workflows (`docker.yml`, `pypi.yaml`) trigger on `release: published`
- The `Cargo.lock` file must remain committed and tracked in git
- Version patching happens in CI via `sed` on `Cargo.toml` — do not manually sync versions across files
