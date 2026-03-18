#!/usr/bin/env bash
# release.sh <version>
# After merging a PR (which triggers CI to build + publish SDKs + create GitHub release),
# run this to update the homebrew-tap formula with real SHA256s.
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

echo "==> Updating homebrew-tap for $TAG"

# ── 1. Verify release exists and is published ────────────────────────────────

echo "--> Checking GitHub release..."
DRAFT=$(gh release view "$TAG" --repo "$REPO" --json isDraft --jq '.isDraft' 2>/dev/null || echo "not_found")

if [ "$DRAFT" = "not_found" ]; then
  echo "ERROR: Release $TAG not found. Wait for CI to complete."
  exit 1
fi

if [ "$DRAFT" = "true" ]; then
  echo "    Release is a draft — publishing..."
  gh release edit "$TAG" --draft=false --repo "$REPO"
  echo "    Published."
fi

# ── 2. Download artifacts and compute hashes ─────────────────────────────────

echo "--> Downloading release artifacts"
TMPDIR=$(mktemp -d)
cd "$TMPDIR"

gh release download "$TAG" \
  --pattern "bandito-aarch64-apple-darwin.tar.gz" \
  --pattern "bandito-x86_64-apple-darwin.tar.gz" \
  --pattern "bandito-x86_64-unknown-linux-gnu.tar.gz" \
  --repo "$REPO"

hash_file() {
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

# ── 3. Rewrite homebrew-tap formula ──────────────────────────────────────────

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

# ── 4. Commit and push homebrew-tap ──────────────────────────────────────────

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
