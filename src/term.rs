static mut TERM_SIZE: Option<(u16, u16)> = None;
const DEFAULT_SIZE: (u16, u16) = (50, 50); // if dyn. fails

pub fn size<T: From<u16>>() -> (T, T){
  let t = unsafe { TERM_SIZE }.unwrap_or(DEFAULT_SIZE);
  (t.0.into(), t.1.into())
}

pub fn set_size() -> Result<(), String> {
  unsafe { 
    TERM_SIZE = Some(termion::terminal_size().map_err(|e| format!("Failed to get terminal size: {}", e))?);
  }
  
  Ok(())
}