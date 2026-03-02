# Deployment Guide

How to release the CLI, publish the SDKs, and set up the Homebrew tap.

## Overview

| Component | Distribution Channel | Trigger |
|-----------|---------------------|---------|
| CLI binary | GitHub Releases + Homebrew | Git tag `v*` |
| Python SDK | PyPI | Git tag `v*` (automated) |
| JS SDK | npm | Manual (future: tag-triggered) |
| Rust engine | Not published standalone | Consumed internally by CLI + SDKs |

## CLI Release

### 1. Tag a version

```bash
git tag v0.1.0
git push origin v0.1.0
```

### 2. Release workflow runs automatically

The `release.yml` workflow triggers on any `v*` tag push. It:

1. Builds `bandito-cli` in release mode for 4 targets:
   - `x86_64-apple-darwin` (macOS Intel)
   - `aarch64-apple-darwin` (macOS Apple Silicon)
   - `x86_64-unknown-linux-gnu` (Linux)
   - `x86_64-pc-windows-msvc` (Windows)
2. Packages each binary (`tar.gz` for unix, `zip` for Windows)
3. Creates a GitHub Release with auto-generated release notes
4. Uploads all 4 archives as release assets

### 3. Verify the release

Check `https://github.com/bandito-ai/bandito/releases` for the new release with all 4 binaries attached.

### Building locally (any platform)

```bash
cargo build -p bandito-cli --release
# Binary at target/release/bandito
```

## Homebrew Tap

### First-time setup

1. Create the `bandito-ai/homebrew-tap` repo on GitHub
2. Copy the formula into it:

```bash
mkdir -p Formula
cp homebrew-formula/bandito.rb Formula/bandito.rb
```

3. After the first release, download each binary and compute SHA256 hashes:

```bash
# For each platform archive:
shasum -a 256 bandito-aarch64-apple-darwin.tar.gz
shasum -a 256 bandito-x86_64-apple-darwin.tar.gz
shasum -a 256 bandito-x86_64-unknown-linux-gnu.tar.gz
```

4. Replace the `PLACEHOLDER_*` values in `Formula/bandito.rb` with the real hashes
5. Commit and push to `bandito-ai/homebrew-tap`

### Updating for new releases

After each new CLI release:

1. Update `version` in `Formula/bandito.rb`
2. Download the new release archives and recompute SHA256 hashes
3. Update the hashes in the formula
4. Commit and push

### User install

```bash
brew install bandito-ai/tap/bandito
```

## Python SDK (PyPI)

The Python SDK uses maturin as its build backend. The Rust engine is compiled into the package automatically — there is no separate `bandito-engine` wheel.

Publishing is fully automated via the `publish-python.yml` workflow, which triggers on any `v*` tag push (same trigger as the CLI release).

### What the workflow does

1. Builds platform-specific wheels using `PyO3/maturin-action` for 5 targets:
   - Linux x86_64 + aarch64 (via `manylinux`)
   - macOS x86_64 (Intel) + aarch64 (Apple Silicon)
   - Windows x86_64
2. Builds a source distribution (`sdist`) for fallback/editable installs
3. Publishes all wheels + sdist to PyPI using trusted publisher (OIDC)

### First-time setup (manual)

Before the first publish, you must:

1. Register the `bandito` package on [PyPI](https://pypi.org)
2. Configure trusted publisher on PyPI: Settings → Publishing → Add GitHub Actions publisher:
   - Repository: `bandito-ai/bandito`
   - Workflow: `publish-python.yml`
   - Environment: `pypi`
3. Create a `pypi` environment in GitHub repo settings (recommended for approval gates)

### Building locally

```bash
cd sdks/python
maturin build --release    # builds .whl with embedded Rust extension
```

### Version bumping

Update the version in `sdks/python/pyproject.toml`:

```toml
[project]
version = "0.1.0"  # bump this
```

## JavaScript SDK (npm)

### Prerequisites

- npm account with publish access to `bandito` package
- WASM engine built

### Build WASM engine

```bash
cd engine
wasm-pack build --target nodejs --out-dir pkg --features wasm
```

### Build and publish

```bash
cd sdks/javascript
pnpm install
pnpm build          # CJS + ESM via tsup
pnpm publish        # or: npm publish
```

### Version bumping

Update the version in `sdks/javascript/package.json`:

```json
{
  "version": "0.1.0"
}
```

## CI Workflows

All workflows live in `.github/workflows/`:

| Workflow | Triggers on | What it does |
|----------|------------|-------------|
| `engine.yml` | Push to `engine/` | Rust tests, WASM build |
| `cli.yml` | Push to `cli/` or `engine/` | Build + test CLI on ubuntu + macOS |
| `sdk-python.yml` | Push to `sdks/python/` or `engine/` | `uv sync` (builds Rust engine via maturin), run pytest |
| `sdk-javascript.yml` | Push to `sdks/javascript/` or `engine/` | Build WASM, run vitest |
| `release.yml` | Tag `v*` | Build CLI binaries for 4 platforms, create GitHub Release |
| `publish-python.yml` | Tag `v*` | Build Python wheels for 5 platforms, publish to PyPI |

## Version Strategy

All components share the same version number tied to git tags:

- `Cargo.toml` workspace members (`engine/Cargo.toml`, `cli/Cargo.toml`)
- `sdks/python/pyproject.toml`
- `sdks/javascript/package.json`
- `homebrew-formula/bandito.rb`

Bump all of these before tagging a release.

## Checklist: Cutting a Release

1. Update version in all 5 files listed above
2. Commit: `git commit -m "Bump version to 0.2.0"`
3. Tag: `git tag v0.2.0`
4. Push: `git push origin main v0.2.0`
5. Wait for `release.yml` to complete — verify 4 binaries on GitHub Releases
6. Wait for `publish-python.yml` to complete — verify new version on [PyPI](https://pypi.org/project/bandito/)
7. Update Homebrew formula with new version + SHA256 hashes
8. Publish JS SDK to npm: `cd sdks/javascript && pnpm build && pnpm publish`
