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
