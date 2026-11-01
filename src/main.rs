fn main() -> std::process::ExitCode {
  match cldr::cli::run(std::env::args().skip(1).collect()) {
    Ok(()) => std::process::ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("cldr: {}", cldr::clean(&error.to_string()));
      std::process::ExitCode::FAILURE
    }
  }
}
