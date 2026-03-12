class Bandito < Formula
  desc "CLI and TUI grading workbench for Bandito — contextual bandit optimizer for LLM selection"
  homepage "https://bandito.dev"
  version "0.1.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/bandito-ai/bandito/releases/download/v#{version}/bandito-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_ARM64"
    else
      url "https://github.com/bandito-ai/bandito/releases/download/v#{version}/bandito-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_X86_64"
    end
  end

  on_linux do
    url "https://github.com/bandito-ai/bandito/releases/download/v#{version}/bandito-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "PLACEHOLDER_LINUX"
  end

  def install
    bin.install "bandito"
  end

  test do
    assert_match "bandito", shell_output("#{bin}/bandito --version")
  end
end
