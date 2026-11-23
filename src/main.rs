fn main() -> std::process::ExitCode {
  match cldr::cli::run(std::env::args().skip(1).collect(), version_line) {
    Ok(()) => std::process::ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("cldr: {}", cldr::clean(&error.to_string()));
      std::process::ExitCode::FAILURE
    }
  }
}

// The version line, docs/dev-opt-brew.md's T4: in an opt or brew tree, the
// `line` its manifest records; otherwise the line bin/devbin build stamped;
// either under this binary's own name. The contract's reader word for word, its
// nested ifs written as the one let chain that Clippy's collapsible_if asks of an
// edition 2024 crate (devbin#0326, cldr#0001).
fn version_line() -> String {
  const TOOL: &str = "cldr";
  const NAME: &str = env!("CARGO_BIN_NAME");
  const MANIFEST: &str = ".cldr-install";
  let named = |line: &str| match line.split_once(' ') {
    Some((_, rest)) => format!("{NAME} {rest}"),
    None => line.to_string(),
  };
  let tree = std::env::current_exe().and_then(|p| p.canonicalize());
  if let Some(root) = tree
    .ok()
    .as_deref()
    .and_then(|p| p.parent())
    .and_then(|p| p.parent())
    && let Ok(text) = std::fs::read_to_string(root.join(MANIFEST))
    && text.lines().any(|l| l == format!("# tool: {TOOL}"))
    && let Some(line) = text.lines().find_map(|l| l.strip_prefix("# line: "))
  {
    return named(line);
  }
  match option_env!("DEVBIN_VERSION_LINE") {
    Some(line) if !line.is_empty() => named(line),
    _ => format!("{NAME} {} (unknown, unknown)", env!("CARGO_PKG_VERSION")),
  }
}
