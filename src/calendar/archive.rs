//! Keyed archives, as the store keeps titles, calendar names and a rule's lists.

use plist::Value;
use std::io::Cursor;

fn objects(blob: &[u8]) -> Option<(Vec<Value>, Option<Value>)> {
  // Some blobs carry a one-byte prefix before the property list.
  let blob = match blob.first() {
    Some(1) => &blob[1..],
    _ => blob,
  };
  let value = Value::from_reader(Cursor::new(blob)).ok()?;
  let dict = value.into_dictionary()?;
  let root = dict
    .get("$top")
    .and_then(|top| top.as_dictionary())
    .and_then(|top| top.get("root"))
    .cloned();
  let objects = dict.get("$objects").and_then(|o| o.as_array()).cloned()?;
  Some((objects, root))
}

/// The plain string inside an archived (attributed) string.
pub fn string(blob: &[u8]) -> Option<String> {
  let (objects, root) = objects(blob)?;
  resolve(&objects, &root?, 0)
}

fn resolve(objects: &[Value], value: &Value, depth: usize) -> Option<String> {
  // An archive is a graph; a bound keeps a malformed one from looping.
  if depth > objects.len() {
    return None;
  }
  let value = match value {
    Value::Uid(uid) => objects.get(usize::try_from(uid.get()).ok()?)?,
    other => other,
  };
  match value {
    Value::String(text) => Some(text.clone()),
    Value::Dictionary(dict) => ["NSString", "NS.string"]
      .iter()
      .find_map(|key| dict.get(key))
      .and_then(|inner| resolve(objects, inner, depth + 1)),
    _ => None,
  }
}

/// The integers in an archived list (a rule's days, months and positions).
pub fn integers(blob: &[u8]) -> Vec<i64> {
  match objects(blob) {
    Some((objects, _)) => objects
      .iter()
      .skip(1)
      .filter_map(|v| v.as_signed_integer())
      .collect(),
    None => Vec::new(),
  }
}

#[doc(hidden)]
pub mod build {
  //! Archives for tests, in the shape the store uses: the unit tests' and `tests/cli.rs`'s one
  //! builder, so it is public but hidden, and no part of cldr's own reading.
  use crate::Result;
  use plist::{Dictionary, Uid, Value};

  fn write(objects: Vec<Value>) -> Result<Vec<u8>> {
    let mut top = Dictionary::new();
    top.insert("root".into(), Value::Uid(Uid::new(1)));
    let mut dict = Dictionary::new();
    dict.insert("$archiver".into(), Value::String("NSKeyedArchiver".into()));
    dict.insert("$version".into(), Value::Integer(100_000.into()));
    dict.insert("$top".into(), Value::Dictionary(top));
    dict.insert("$objects".into(), Value::Array(objects));
    let mut out = Vec::new();
    Value::Dictionary(dict).to_writer_binary(&mut out)?;
    Ok(out)
  }

  /// A string wrapped the way an attributed string is.
  pub fn string(text: &str) -> Result<Vec<u8>> {
    let mut wrapper = Dictionary::new();
    wrapper.insert("NSString".into(), Value::Uid(Uid::new(2)));
    write(vec![
      Value::String("$null".into()),
      Value::Dictionary(wrapper),
      Value::String(text.into()),
    ])
  }

  pub fn integers(values: &[i64]) -> Result<Vec<u8>> {
    let mut objects = vec![Value::String("$null".into())];
    objects.extend(values.iter().map(|v| Value::Integer((*v).into())));
    write(objects)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_wrapped_string_reads_back() -> crate::Result<()> {
    let text = "Weekly sync, room 2";
    assert_eq!(string(&build::string(text)?).as_deref(), Some(text));
    Ok(())
  }

  #[test]
  fn the_prefix_byte_is_ignored() -> crate::Result<()> {
    let text = "Prefixed";
    let mut blob = vec![1];
    blob.extend(build::string(text)?);
    assert_eq!(string(&blob).as_deref(), Some(text));
    Ok(())
  }

  #[test]
  fn integer_lists_read_back_in_order() -> crate::Result<()> {
    let values = [1, 3, 5];
    assert_eq!(integers(&build::integers(&values)?), values);
    Ok(())
  }

  #[test]
  fn garbage_is_none_not_a_panic() {
    assert_eq!(string(b"not a plist"), None);
    assert!(integers(b"").is_empty());
  }
}
