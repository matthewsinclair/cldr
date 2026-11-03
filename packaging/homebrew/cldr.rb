# packaging/homebrew/cldr.rb: the template of cldr's Homebrew formula.
# bin/devbin formula render fills it from a release: @@VERSION@@, the asset's
# @@SHA256:...@@ from the release's SHA256SUMS, and @@FINISH@@ (0755 on
# bin/cldr, the manifest, the link in bin). This header is dropped from the
# render. Edit this file, never a rendered one.
#
# REQUIREMENT repeats packaging/macos/signing's designated requirement, which
# a Ruby formula cannot read.
class Cldr < Formula
  desc "Re-runnable renderer for a plain-text time journal"
  homepage "https://github.com/matthewsinclair/cldr"
  url "https://github.com/matthewsinclair/cldr/releases/download/v@@VERSION@@/cldr-@@VERSION@@-aarch64-apple-darwin.tar.gz"
  sha256 "@@SHA256:cldr-{version}-aarch64-apple-darwin.tar.gz@@"
  license "MIT"

  # macOS on Apple silicon: cldr reads BusyCal's store
  depends_on arch: :arm64
  depends_on :macos

  # The designated requirement cldr is signed to: its identifier, then a
  # Developer ID certificate for Geodica's team
  REQUIREMENT = "identifier \"com.geodica.cldr\" and anchor apple generic " \
                "and certificate 1[field.1.2.840.113635.100.6.2.6] " \
                "and certificate leaf[field.1.2.840.113635.100.6.1.13] " \
                "and certificate leaf[subject.OU] = \"76BQL8L47U\"".freeze

  def verify_signature
    system "/usr/bin/codesign", "--verify", "--strict", "-R=#{REQUIREMENT}", libexec/"bin/cldr"
  end

  def install
    # The binary into libexec and the example config into share/cldr, then
    # devbin's finish: 0755 on bin/cldr, the manifest and the link in bin
    libexec.install "bin"
    pkgshare.install "config.toml"
    @@FINISH@@
    verify_signature
  end

  test do
    verify_signature
    assert_match version.to_s, shell_output("#{bin}/cldr --version")
    (testpath/"config.toml").write("[journal]\n")
    assert_match "config", shell_output("#{bin}/cldr --config #{testpath}/config.toml paths")
  end
end
