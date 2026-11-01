pub mod calendar;
pub mod check;
pub mod cli;
pub mod config;
pub mod doctor;
pub mod export;
pub mod journal;
pub mod legacy;
pub mod model;
pub mod month;
pub mod names;
pub mod notes;
pub mod period;
pub mod render;
pub mod zones;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

/// Never let configuration or calendar text inject terminal control sequences.
pub fn clean(text: &str) -> String {
  text.chars().filter(|c| !c.is_control()).collect()
}
