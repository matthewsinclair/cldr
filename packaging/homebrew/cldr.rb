# Canonical template. tools/formula renders it for a tag; never fill these in by hand.
class Cldr < Formula
  desc "Re-runnable renderer for a plain-text time journal"
  homepage "https://github.com/matthewsinclair/cldr"
  url "https://github.com/matthewsinclair/cldr/archive/refs/tags/@RELEASE_TAG@.tar.gz"
  sha256 "@RELEASE_SHA256@"
  license "MIT"
  head "https://github.com/matthewsinclair/cldr.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
    pkgshare.install "examples/config.toml"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/cldr --version")
    (testpath/"config.toml").write("[journal]\n")
    assert_match "config", shell_output("#{bin}/cldr --config #{testpath}/config.toml paths")
  end
end
