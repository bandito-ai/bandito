# Deployment Guide

How to release the CLI, publish the SDKs, and set up the Homebrew tap.

## Overview

| Component | Distribution Channel | Trigger |
|-----------|---------------------|---------|
| CLI binary | GitHub Releases + Homebrew | Git tag `v*` |
| Python SDK | PyPI | Git tag `v*` (automated) |
| JS SDK | npm | Git tag `v*` (automated) |
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
3. Publishes all wheels + sdist to PyPI using API token (`PYPI_API_TOKEN` secret)

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

Publishing is fully automated via the `publish-javascript.yml` workflow, which triggers on any `v*` tag push.

### What the workflow does

1. Builds WASM engine (`wasm-pack build`)
2. Builds SDK (`pnpm build` via tsup → CJS + ESM)
3. Publishes to npm using `NPM_TOKEN` secret

### First-time setup

1. Create an npm access token: `npm token create` (or npmjs.com → Access Tokens → Generate)
2. Add it as `NPM_TOKEN` in GitHub repo secrets

### Building locally

```bash
cd engine && wasm-pack build --target nodejs --out-dir pkg --features wasm
cd ../sdks/javascript && pnpm install && pnpm build
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
| `publish-javascript.yml` | Tag `v*` | Build WASM + SDK, publish to npm |

## Version Strategy

All components share the same version number tied to git tags:

- `Cargo.toml` workspace members (`engine/Cargo.toml`, `cli/Cargo.toml`)
- `sdks/python/pyproject.toml`
- `sdks/javascript/package.json`
- `homebrew-formula/bandito.rb`

Bump all of these before tagging a release.

## Checklist: Cutting a Release

Example below uses `0.2.0` — replace with your version.

### 1. Bump version in all 5 files

```bash
# engine/Cargo.toml        → version = "0.2.0"
# cli/Cargo.toml            → version = "0.2.0"
# sdks/python/pyproject.toml → version = "0.2.0"
# sdks/javascript/package.json → "version": "0.2.0"
# homebrew-formula/bandito.rb → version "0.2.0"
```

### 2. Commit, tag, push

```bash
git add engine/Cargo.toml cli/Cargo.toml sdks/python/pyproject.toml sdks/javascript/package.json homebrew-formula/bandito.rb
git commit -m "v0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

### 3. Wait for CI (5-10 min)

Two workflows trigger automatically on the `v*` tag:

- `release.yml` → builds CLI binaries for 4 platforms, creates GitHub Release
- `publish-python.yml` → builds Python wheels for 5 platforms, publishes to PyPI

Check status:

```bash
gh run list --limit 6
```

### 4. Verify

- GitHub Releases: `https://github.com/bandito-ai/bandito/releases`  — 4 binaries attached
- PyPI: `https://pypi.org/project/bandito/` — new version visible
- Quick test: `pip install bandito==0.2.0`

### 5. Update Homebrew formula

Download the macOS release archives and compute SHA256 hashes:

```bash
# Download from GitHub Releases
curl -LO https://github.com/bandito-ai/bandito/releases/download/v0.2.0/bandito-aarch64-apple-darwin.tar.gz
curl -LO https://github.com/bandito-ai/bandito/releases/download/v0.2.0/bandito-x86_64-apple-darwin.tar.gz
curl -LO https://github.com/bandito-ai/bandito/releases/download/v0.2.0/bandito-x86_64-unknown-linux-gnu.tar.gz

# Compute hashes
shasum -a 256 bandito-*.tar.gz
```

Update hashes in `homebrew-formula/bandito.rb`, commit, and push to `bandito-ai/homebrew-tap`.

### 6. Verify JS SDK

Check `https://www.npmjs.com/package/bandito` for the new version.

### Gotchas

- **PyPI versions are permanent.** You can't re-upload the same version. If a publish partially fails, bump the version.
- **`macos-latest`** is ARM (Apple Silicon). x86_64 macOS builds cross-compile on the same runner.
- **PyPI auth** uses `PYPI_API_TOKEN` GitHub secret (not trusted publisher).
- **npm auth** uses `NPM_TOKEN` GitHub secret.
- **Cargo.lock** will update when you bump `engine/Cargo.toml` — make sure to include it in the commit.
