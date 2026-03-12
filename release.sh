#!/usr/bin/env bash
# release.sh <version>
# Bumps version, tags, waits for CI, updates homebrew-tap.
# Usage: ./release.sh 0.1.1
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>   (e.g. $0 0.1.1)"
  exit 1
fi

VERSION="$1"
TAG="v$VERSION"
REPO="bandito-ai/bandito"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TAP_DIR="$SCRIPT_DIR/../homebrew-tap"

echo "==> Releasing $TAG"

# ── 1. Bump version in all 5 files ───────────────────────────────────────────

echo "--> Bumping version to $VERSION"

sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$SCRIPT_DIR/engine/Cargo.toml"
sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$SCRIPT_DIR/cli/Cargo.toml"
sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$SCRIPT_DIR/sdks/python/pyproject.toml"

# package.json uses a different key format
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" "$SCRIPT_DIR/sdks/javascript/package.json"

# homebrew-formula source (for reference; tap is updated separately below)
sed -i '' "s/version \"[^\"]*\"/version \"$VERSION\"/" "$SCRIPT_DIR/homebrew-formula/bandito.rb"

# ── 2. Commit, tag, push ─────────────────────────────────────────────────────

echo "--> Committing and tagging $TAG"
cd "$SCRIPT_DIR"
git add -A
git status --short
git commit -m "$TAG"
git tag "$TAG"
git push origin main "$TAG"

# ── 3. Wait for Release CI ───────────────────────────────────────────────────

echo "--> Waiting for Release workflow to start..."
RUN_ID=""
for i in $(seq 1 30); do
  RUN_ID=$(gh run list \
    --workflow=release.yml \
    --repo "$REPO" \
    --limit 10 \
    --json databaseId,headBranch \
    --jq ".[] | select(.headBranch == \"$TAG\") | .databaseId" 2>/dev/null | head -1)
  if [ -n "$RUN_ID" ]; then
    echo "    Run ID: $RUN_ID"
    break
  fi
  echo "    Not started yet... ($i/30)"
  sleep 3
done

if [ -z "$RUN_ID" ]; then
  echo "ERROR: Release workflow did not start within 90 seconds."
  exit 1
fi

echo "--> Watching Release CI (this takes ~5 min)..."
gh run watch "$RUN_ID" --repo "$REPO"

# Confirm it succeeded
STATUS=$(gh run view "$RUN_ID" --repo "$REPO" --json conclusion --jq '.conclusion')
if [ "$STATUS" != "success" ]; then
  echo "ERROR: Release workflow concluded with status: $STATUS"
  echo "       Check: https://github.com/$REPO/actions/runs/$RUN_ID"
  exit 1
fi

# ── 4. Download artifacts and compute hashes ─────────────────────────────────

echo "--> Downloading release artifacts"
TMPDIR=$(mktemp -d)
cd "$TMPDIR"

gh release download "$TAG" \
  --pattern "bandito-aarch64-apple-darwin.tar.gz" \
  --pattern "bandito-x86_64-apple-darwin.tar.gz" \
  --pattern "bandito-x86_64-unknown-linux-gnu.tar.gz" \
  --repo "$REPO"

hash_file() {
  # Works on both macOS (shasum) and Linux (sha256sum)
  if command -v sha256sum &>/dev/null; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

ARM64=$(hash_file "bandito-aarch64-apple-darwin.tar.gz")
X86_64=$(hash_file "bandito-x86_64-apple-darwin.tar.gz")
LINUX=$(hash_file "bandito-x86_64-unknown-linux-gnu.tar.gz")

echo "    aarch64-apple-darwin:       $ARM64"
echo "    x86_64-apple-darwin:        $X86_64"
echo "    x86_64-unknown-linux-gnu:   $LINUX"

# ── 5. Rewrite homebrew-tap formula ──────────────────────────────────────────

echo "--> Updating $TAP_DIR/Formula/bandito.rb"

python3 - "$VERSION" "$ARM64" "$X86_64" "$LINUX" "$TAP_DIR" << 'EOF'
import sys

version, arm64, x86_64, linux, tap_dir = sys.argv[1:]

formula = f"""\
class Bandito < Formula
  desc "CLI and TUI grading workbench for Bandito \u2014 contextual bandit optimizer for LLM selection"
  homepage "https://bandito.dev"
  version "{version}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/bandito-ai/bandito/releases/download/v{version}/bandito-aarch64-apple-darwin.tar.gz"
      sha256 "{arm64}"
    else
      url "https://github.com/bandito-ai/bandito/releases/download/v{version}/bandito-x86_64-apple-darwin.tar.gz"
      sha256 "{x86_64}"
    end
  end

  on_linux do
    url "https://github.com/bandito-ai/bandito/releases/download/v{version}/bandito-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "{linux}"
  end

  def install
    bin.install "bandito"
  end

  test do
    assert_match "bandito", shell_output("\#{bin}/bandito --version")
  end
end
"""

path = f"{tap_dir}/Formula/bandito.rb"
with open(path, "w") as f:
    f.write(formula)
print(f"    Written: {path}")
EOF

# ── 6. Commit and push homebrew-tap ──────────────────────────────────────────

echo "--> Pushing homebrew-tap"
cd "$TAP_DIR"
git add Formula/bandito.rb
git commit -m "bandito $TAG"
git push

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo "Done. $TAG is live."
echo ""
echo "Verify:"
echo "  brew update && brew upgrade bandito-ai/tap/bandito"
echo "  bandito --version"
