class Klef < Formula
  desc "Local-first vault for API keys, backed by the OS keychain"
  homepage "https://github.com/slewinus/klef"
  version "__VERSION__"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/slewinus/klef/releases/download/v#{version}/klef-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "__SHA_AARCH64_APPLE_DARWIN__"
    else
      url "https://github.com/slewinus/klef/releases/download/v#{version}/klef-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "__SHA_X86_64_APPLE_DARWIN__"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/slewinus/klef/releases/download/v#{version}/klef-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "__SHA_AARCH64_UNKNOWN_LINUX_GNU__"
    else
      url "https://github.com/slewinus/klef/releases/download/v#{version}/klef-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "__SHA_X86_64_UNKNOWN_LINUX_GNU__"
    end
  end

  def install
    bin.install "klef"
    # Generate and install shell completions while we have a runnable binary.
    generate_completions_from_executable(bin/"klef", "completions")
  end

  test do
    assert_match "klef", shell_output("#{bin}/klef --version")
    assert_match "Local-first vault", shell_output("#{bin}/klef --help")
  end
end
