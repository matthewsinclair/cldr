# packaging/homebrew/cldr.rb: the template of cldr's Homebrew formula.
# bin/devbin formula render fills it from a release: @@VERSION@@, the asset's
# @@SHA256:...@@ from the release's SHA256SUMS, and @@INSTALL@@, the install
# body devbin generates from the install: block in bin/.devbin/config.yaml
# (devbin#0326, cldr#0001: the tree into libexec, 0755 on bin/cldr, the T3
# manifest with its line and managed_by: brew, the link in bin). This header
# is dropped from the render. Edit this file, never a rendered one.
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
    @@INSTALL@@
    # The example config, where README.md says it is: share/cldr/config.toml
    pkgshare.install_symlink libexec/"config.toml"
    verify_signature
  end

  def caveats
    <<~EOS
      If you also run cldr from a checkout, `bin/devbin use brew`, run in that
      checkout, hands the cldr command back to Homebrew.
    EOS
  end

  test do
    verify_signature
    assert_match "cldr #{version} (brew, ", shell_output("#{bin}/cldr --version")
    (testpath/"config.toml").write("[journal]\n")
    assert_match "config", shell_output("#{bin}/cldr --config #{testpath}/config.toml paths")
  end
end
