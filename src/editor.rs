use std::io::{self, Write};

extern crate termion;
use termion::raw::IntoRawMode;
use termion::event::Key;
use termion::input::TermRead;

use unicode_width::UnicodeWidthStr;

use crate::term;

fn draw_lines_with_exclusions(lines: &Vec<String>, exclude: &Vec<bool>, cursor_line: usize, stdout: &mut termion::raw::RawTerminal<io::Stdout>) -> Result<(), String> {
  let tlines = term::size::<usize>().1.saturating_sub(1);
  let tcols = term::size::<usize>().0.saturating_sub(1);

  let extralinesbeforecursor = lines.iter().take(cursor_line).map(|l| l.width() / tcols).sum::<usize>();
  let scrcursorline = cursor_line + extralinesbeforecursor;
  let scrlowcutoff = scrcursorline - (scrcursorline % tlines);
  let extralinesbeforescreen = lines.iter().take(scrlowcutoff).map(|l| l.width() / tcols).sum::<usize>();
  
  let lowcutoff = scrlowcutoff - extralinesbeforescreen;
  let highcutoff = lowcutoff + tlines;
  
  write!(stdout, "{}{}", termion::clear::All, termion::cursor::Goto(1, 1)).map_err(|e| format!("Failed to write: {}", e))?;

  let mut real_lines_done = 0;
  
  for i in lowcutoff..highcutoff {
    if i >= lines.len() || real_lines_done >= tlines {
      break;
    }

    let (line, excl) = (&lines[i], &exclude[i]);
    
    if *excl {
      write!(stdout, "\x1b[41m").map_err(|e| format!("Failed to write: {}", e))?;
    }
    if cursor_line == i {
      write!(stdout, "\x1b[4m").map_err(|e| format!("Failed to write: {}", e))?;
    }
    write!(stdout, "{:10}", line).map_err(|e| format!("Failed to write: {}", e))?;
    write!(stdout, "\x1b[0m\n\r").map_err(|e| format!("Failed to write: {}", e))?;

    real_lines_done += line.width() / tcols + 1;
  }
  
  stdout.flush().map_err(|e| format!("Failed to write: {}", e))?;
  
  return Ok(());
}

pub fn rl_editor(lines: &Vec<String>) -> Result<Vec<String>, String> {
  let mut is_deleted: Vec<bool> = vec![false; lines.len()];

  let mut stdout = io::stdout().into_raw_mode().map_err(|e| format!("Failed to enter raw mode.  Trace:\n\r{}", e))?;
  let stdin = io::stdin();
  
  let mut cursor_line = 0;
  write!(stdout, "{}", termion::cursor::Hide).map_err(|e| format!("Failed to write: {}", e))?;
  stdout.flush().map_err(|e| format!("Failed to write: {}", e))?;

  draw_lines_with_exclusions(&lines, &is_deleted, cursor_line, &mut stdout)?;

  for c in stdin.keys() {
    let key = match c {
      Err(e) => {
        drop(stdout);
        
        return Err(format!("\x1b[31mERR:\x1b[0m Readline failed — {}.", e));
      },
      Ok(o) => o
    };
  
    match key {
      Key::Char('\n') => { break; },
      Key::Backspace => {
        is_deleted[cursor_line] = !is_deleted[cursor_line];
      },
      Key::Up => if cursor_line > 0 {
        cursor_line -= 1; 
      },
      Key::Down => if cursor_line + 1 < lines.len() {
        cursor_line += 1;
      },
      Key::Ctrl('c') | Key::Esc => Err("Cancelled write.")?,
      _ => {}
    };

    draw_lines_with_exclusions(&lines, &is_deleted, cursor_line, &mut stdout)?;
  }

  drop(stdout);
  
  let mut to_drop = is_deleted.into_iter();
  let mut newlines = lines.clone();
  newlines.retain(|_| !to_drop.next().unwrap());
  
  Ok(newlines)
}