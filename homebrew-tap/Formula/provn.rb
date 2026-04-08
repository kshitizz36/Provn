class Provn < Formula
  desc "AI-powered secret & IP leak detection — stops threats before they leave your machine"
  homepage "https://github.com/kshitizz36/Provn"
  version "0.0.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/kshitizz36/Provn/releases/download/v#{version}/provn-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_AARCH64_MACOS"
    end
    on_intel do
      url "https://github.com/kshitizz36/Provn/releases/download/v#{version}/provn-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_X86_64_MACOS"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/kshitizz36/Provn/releases/download/v#{version}/provn-aarch64-linux.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_AARCH64_LINUX"
    end
    on_intel do
      url "https://github.com/kshitizz36/Provn/releases/download/v#{version}/provn-x86_64-linux.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_X86_64_LINUX"
    end
  end

  def install
    bin.install "provn"
  end

  test do
    assert_match "provn", shell_output("#{bin}/provn --version")
  end
end
