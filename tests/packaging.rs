//! The Homebrew formula repeats the designated requirement cldr is signed to,
//! since a Ruby formula cannot read packaging/macos/signing; this holds the
//! copy to the one place the fact lives.

#[test]
fn the_formula_checks_the_requirement_the_signer_signs_to() {
  let formula = include_str!("../packaging/homebrew/cldr.rb");
  let signing = include_str!("../packaging/macos/signing");
  // REQUIREMENT is one Ruby string, written as adjacent literals over lines
  let start = formula.find("REQUIREMENT = ").unwrap_or_default();
  let block = &formula[start..];
  let block = &block[..block.find(".freeze").unwrap_or_default()];
  let requirement: String = block
    .lines()
    .map(|l| {
      l.trim()
        .trim_start_matches("REQUIREMENT = ")
        .trim_end_matches('\\')
        .trim()
    })
    .map(|l| {
      l.strip_prefix('"')
        .and_then(|l| l.strip_suffix('"'))
        .unwrap_or(l)
        .replace("\\\"", "\"")
    })
    .collect();
  let signed = signing
    .lines()
    .find_map(|l| l.strip_prefix("requirement.cldr="));
  assert_eq!(signed, Some(requirement.as_str()));
}

/// The formula installs through devbin's one form (devbin#0326, cldr#0001):
/// one @@INSTALL@@, which devbin generates from the install: block, and none
/// of @@FINISH@@, which devbin retired; its test reads the brew tree's line.
#[test]
fn the_formula_installs_through_devbins_one_form() {
  let formula = include_str!("../packaging/homebrew/cldr.rb");
  let body = &formula[formula.find("class Cldr").unwrap_or_default()..];
  assert_eq!(body.matches("@@INSTALL@@").count(), 1, "{body}");
  assert!(!formula.contains("@@FINISH@@"), "{formula}");
  assert!(
    body.contains("assert_match \"cldr #{version} (brew, \""),
    "{body}"
  );
  assert!(body.contains("bin/devbin use brew"), "{body}");
}

/// cldr's publish and switch are devbin's (devbin#0326, cldr#0001): the
/// install: block in bin/.devbin/config.yaml declares cldr's link, and
/// bin/devbin use, run in this checkout, reads it. Under a HOME of its own there
/// is no link, so the report says brew answers for cldr, whether or not this
/// machine has Homebrew's cldr (which decides only its exit).
#[test]
fn bin_devbin_use_reads_cldrs_link_set_from_its_install_block()
-> Result<(), Box<dyn std::error::Error>> {
  let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  let home = std::env::temp_dir().join(format!("cldr-packaging-use-{}", std::process::id()));
  std::fs::create_dir_all(&home)?;
  let out = std::process::Command::new(checkout.join("bin/devbin"))
    .arg("use")
    .env("HOME", &home)
    .current_dir(checkout)
    .stdin(std::process::Stdio::null())
    .output();
  std::fs::remove_dir_all(&home)?;
  let out = out?;
  let stdout = String::from_utf8(out.stdout)?;
  let first = stdout.lines().next().unwrap_or_default();
  assert!(
    first == "use: no link for cldr, so brew"
      || first.starts_with("use: no link for cldr, so brew, and "),
    "bin/devbin use did not read cldr's link set: {stdout}{}",
    String::from_utf8_lossy(&out.stderr)
  );
  Ok(())
}
