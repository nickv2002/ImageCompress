class Imgcompress < Formula
  desc "Compress and convert PNG, JPEG, WebP, GIF, AVIF, JPEG XL and HEIC images"
  homepage "https://nickv2002.github.io/ImageCompress/"
  version "@VERSION@"
  license "GPL-3.0-or-later"

  livecheck do
    url :stable
    strategy :github_latest
  end

  # One universal tarball serves both Mac architectures.
  on_macos do
    on_arm do
      url "https://github.com/nickv2002/ImageCompress/releases/download/v#{version}/imgcompress-#{version}-macos-universal.tar.gz"
      sha256 "@SHA256_MACOS@"
    end
    on_intel do
      url "https://github.com/nickv2002/ImageCompress/releases/download/v#{version}/imgcompress-#{version}-macos-universal.tar.gz"
      sha256 "@SHA256_MACOS@"
    end
  end
  on_linux do
    on_intel do
      url "https://github.com/nickv2002/ImageCompress/releases/download/v#{version}/imgcompress-#{version}-linux-x86_64.tar.gz"
      sha256 "@SHA256_LINUX_X86_64@"
    end
    on_arm do
      url "https://github.com/nickv2002/ImageCompress/releases/download/v#{version}/imgcompress-#{version}-linux-aarch64.tar.gz"
      sha256 "@SHA256_LINUX_AARCH64@"
    end
  end

  def install
    # gifsicle stays private to imgcompress (in libexec) so it never clashes with Homebrew's gifsicle;
    # imgcompress resolves its bin/ link and finds it there.
    libexec.install "imgcompress", "gifsicle"
    (libexec/"licenses").install Dir["licenses/*"] if File.directory?("licenses")
    bin.install_symlink libexec/"imgcompress"
    generate_completions_from_executable(bin/"imgcompress", "completions")
    (man1/"imgcompress.1").write Utils.safe_popen_read(bin/"imgcompress", "man")
  end

  test do
    system bin/"imgcompress", "tools", "--check" # fails if the bundled gifsicle is not found
  end
end
