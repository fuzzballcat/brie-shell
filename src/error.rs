use crate::parse::Location;

use std::fmt;

pub fn location_format<S: AsRef<str>>(s: S, col: usize, len: usize) -> String {
  format!("\x1b[35m|\x1b[0m{0}\n\r{1}\x1b[31m{2}", s.as_ref(), " ".repeat(col + 1), "^".repeat(len))
}

pub fn error<S: AsRef<str>, S2: AsRef<str>, S3: AsRef<str>>(s: S, col: usize, len: usize, errmsg: S2, note: S3) -> String {
  format!(
    "\x1b[31mERR:\x1b[33m {0}\n\r\n\r{1}\n\r\n\r\x1b[35mNote:\x1b[33m {2}\x1b[0m", 
    errmsg.as_ref(), 
    location_format(s, col, len),
    note.as_ref()
  )
}

pub fn internalfailure<E: fmt::Display, S: AsRef<str>, S2: AsRef<str>>(e: E, action: S, s: S2, l: &Location) -> String {
  error(s.as_ref(), l.col, l.len, format!("[INTERNAL] Failed to {}.", action.as_ref()), noteformat!("Trace:\n\r{}", e))
}

// maintain yellow but not on values, values stay white
macro_rules! noteformat {
  // nested format? why not
  ($fmt_str:literal, $($args:expr),*) => {{
    format!($fmt_str, $(format!("\x1b[0m{}\x1b[33m", $args)),*)
  }};
}
pub(crate) use noteformat;