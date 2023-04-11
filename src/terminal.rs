extern crate os_pipe;

extern crate termion;
use termion::raw::IntoRawMode;
use termion::event::Key;
use termion::input::TermRead;

extern crate unicode_segmentation;
use unicode_segmentation::UnicodeSegmentation;

use std::io::{self, Write};
use std::env;
use std::collections::HashMap;

mod charset;
use charset::{Colors, CHARSET};

mod editor;

mod error;
mod eval;

mod parse;
use parse::{AST, ASTVal, Location};

mod pretty;
mod term;
mod token;

const VERSTR: &str = "v0.1";

fn eval_pipeline(command: String, environment: &mut HashMap<String, AST>, allow_command: bool) -> Result<AST, String> {
  let toks = token::tokenize(&command);
        
  let commands = toks.and_then(|mut ts| parse::parse_commands(&mut ts, &environment, &command));

  let resolved = commands.and_then(|cmds| cmds.map(|c| eval::resolve(c, environment, &command)).map_or(Ok(None), |v| v.map(Some)));
  
  let evald = resolved.and_then(|cmds|
    eval::eval_commands(match cmds {
      None => Vec::new(),
      Some(c) => Vec::from([c])
    }, &command, environment, allow_command)
  );

  evald
}

fn main() {
  let mut args: Vec<String> = env::args().collect();
  if args.len() > 1 {
    args.remove(0);
    let filename = args.remove(0);
    if args.len() > 0 {
      println!("\x1b[31mERR:\x1b[0m Unexpected extra argument `{}`", args.remove(0));
      std::process::exit(1);
    }

    match std::fs::read_to_string(&filename) {
      Ok(str) => {
        let mut environment = HashMap::new();
        let l = Location { col: 0, len: 0};
        environment.insert("ARGV".to_string(), AST { variant: ASTVal::Array(args.into_iter().map(|x| AST { variant: ASTVal::String(x), location: l }).collect()), location: l });
        for line in str.split("\n") {
          let result = eval_pipeline(line.to_string(), &mut environment, false);
          if let Err(e) = result {
            println!("{}", e);
            break;
          }
        }
      },
      Err(_e) => { println!("\x1b[31mERR:\x1b[0m Failed to open file `{}`", filename); }
    }
    
    return;
  }
  
  println!("Brie Shell, {}.  `)help` for manual.", VERSTR);
  
  let prompt = "$";
  let mut environment = HashMap::new();
  let mut history = Vec::new();

  let mut replablehistory = Vec::new();
  let mut commit_every = true;
  
  let mut stdout = match io::stdout().into_raw_mode().map_err(|e| format!("Failed to enter raw mode.  Trace:\n\r{}", e)) {
    Ok(o) => o,
    Err(e) => { println!("\x1b[31m{}\x1b[0m", e); std::process::exit(1); }
  };

  match term::set_size() {
    Ok(o) => o,
    Err(e) => { println!("\x1b[31m{}\x1b[0m", e); std::process::exit(1); }
  };
  
  'termloop: loop {
    let stdin = io::stdin();

    history.push(String::new());

    let history_len = history.len();
    let immut_history = history.clone();
    
    let command = &mut history[history_len - 1];
    
    let mut history_index = history_len - 1;

    let pathlen;

    // "0,0"
    write!(stdout, "{}", termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
    
    if let Ok(path) = env::current_dir() { 
      let sfmt = format!("{1}{0} ", prompt, path.display());
      pathlen = sfmt.len();
      write!(stdout, "{1}{}{2}", sfmt, termion::color::Fg(termion::color::BrightRed), Colors::Reset).unwrap();
      stdout.flush().unwrap();
    } else {
      write!(stdout, "\x1b[31mERR:\x1b[0m Cannot fetch current directory!\n\r").unwrap();
      stdout.flush().unwrap();
      std::process::exit(1);
    }
    
    let mut xc_pos: usize = 0;
    
    for c in stdin.keys() {
      let (term_cols, _term_rows) = term::size::<u16>();
      
      let key = match c {
        Err(e) => { 
          write!(stdout, "\x1b[31mERR:\x1b[0m Readline failed — {}.", e).unwrap();
          stdout.flush().unwrap();
          continue 'termloop; 
        },
        Ok(o) => o
      };

      let mut is_final = false;
      match key {
        Key::Char('\n') => {
          is_final = true;
        },
        Key::Ctrl('c') => {
          write!(stdout, "\n\r").unwrap();
          std::process::exit(0);
        },
        Key::Char(x) => { 
          if xc_pos >= command.len() {
            command.push(x);
          } else {
            command.insert(xc_pos, x);
          }
          xc_pos += 1;
        },
        Key::Up => {
          if xc_pos >= term_cols as usize {
            xc_pos -= term_cols as usize;
          } else if history_index > 0 {
            history_index -= 1;
            *command = immut_history[history_index].clone();
            
            xc_pos = command.len();
          }
        },
        Key::Down => {
          if xc_pos + term_cols as usize <= command.len() {
            xc_pos += term_cols as usize;
          } else if history_index + 1 < history_len {
            history_index += 1;
            *command = immut_history[history_index].clone();
            
            xc_pos = command.len();
          }
        },
        Key::Left => {
          if xc_pos > 0 {
            xc_pos -= 1;
          }
        },
        Key::Right => {
          if xc_pos < command.len() {
            xc_pos += 1;
          }
        },
        Key::Backspace => {
          if xc_pos <= 0 {
            xc_pos += 1; // counteract
          } else if xc_pos >= command.len() {
            command.pop();
          } else {
            command.replace_range(
              command
                .char_indices()
                .nth(xc_pos - 1)
                .map(|(pos, ch)| (pos..pos + ch.len_utf8()))
                .unwrap(),
              ""
            );
          }
          xc_pos -= 1;
        }
        _ => {}
      }

      let linesup = (xc_pos + pathlen - 1) / (term_cols as usize);
      if linesup > 0 {
        write!(stdout, "{}", termion::cursor::Up(linesup as u16)).unwrap();
      }
      write!(stdout, "\r{}{}", termion::cursor::Right(pathlen as u16), termion::clear::AfterCursor).unwrap();

      let highlight_cmd = pretty::highlight(command);
      
      write!(stdout, "{}", highlight_cmd).unwrap();

      // EXECUTE LIVE-RELOAD

      write!(stdout, "\n\r").unwrap();
      stdout.flush().unwrap();
      
      if !is_final {
        write!(stdout, "{}{}", Colors::Reset, termion::color::Fg(termion::color::Rgb(80, 80, 80))).unwrap();
      }
      
      // exit raw mode for the moment — to make normal term. cmds. work
      drop(stdout);

      let mut linesdown = 0;

      if command.graphemes(true).nth(0).unwrap_or(" ") == CHARSET::EnvCommand {
        let c = command.chars().skip(1).collect::<String>();
        let mut cmd: Vec<String> = c.trim().split(" ").map(|x| x.to_lowercase()).collect();

        if is_final || cmd[0] == "info" {
          match match cmd[0].as_str() {
            "rtf" => {
              if cmd.len() != 2 {
                Err(")rtf expects a single filename to write to.".to_string())
              } else {
                let f = cmd.remove(1);
                let new_rhis = editor::rl_editor(&replablehistory);
                match new_rhis {
                  Err(e) => Err(e),
                  Ok(o) => std::fs::write(f.clone(), o.join("\n")).map(|_| format!("Wrote {} lines to file {}.", o.len(), f)).map_err(|e| format!("Failed to write to file.  Trace: {}", e))
                }
              }
            },
            "cm" => {
              commit_every = !commit_every;
              Ok(format!("Commit mode is now {}.", if commit_every { "automatic" } else { "manual" }))
            },
            "c" => {
              if cmd.len() > 1 {
                replablehistory.push(cmd[1..].join(" "));
              } else if immut_history.len() > 1 {
                let l = immut_history.len() - 2;
                replablehistory.push(
                  immut_history[l].clone()
                );
              }
              Ok("foo".to_string())
            },
            "clear" => {
              Ok(format!("{}{}", termion::clear::All, termion::cursor::Goto(1, 1)))
            },
            "wipe" => {
              environment.clear();
              replablehistory.clear();
              Ok(format!("{}{}", termion::clear::All, termion::cursor::Goto(1, 1)))
            },
            "info" => {
              let torun = cmd[1..].join(" ");
              let res = match token::tokenize(&torun).and_then(|mut c| parse::parse_commands(&mut c, &environment, &torun)) {
                Err(e) => Err(e),
                Ok(o) => Ok(match o {
                  None => "".to_string(),
                  Some(s) => s.to_tree()
                })
              };
              linesdown += res.as_ref().ok().unwrap_or(&String::new()).lines().count();
              res
            },
            "help" => {
              if cmd.len() < 2 {
                Ok(format!("\x1b[0;1mBrie Shell, {}\x1b[32m\nUse `\x1b[0m)help name\x1b[32m` to find info about a specific function, operator, or structure.\nTry `\x1b[0m)help builtins\x1b[32m` for builtin functions or `\x1b[0m)help language\x1b[32m` for a language tutorial.\n\n{}", VERSTR, charset::refcard()))
              } else {
                let c = cmd[1..].join(" "); 
                Ok(format!("\x1b[4mInfo for `\x1b[0;4;1m{}\x1b[0;4;32m`:\x1b[0;32m\n{}", c, charset::detailedinfo(c.as_str())))
              }
            },
            wrong => Err(error::noteformat!("\x1b[31mInvalid shell command {}\x1b[0m", wrong))
          } {
            Err(e) => if is_final { write!(io::stdout(), "\x1b[31m{}\x1b[0m", e) } else { write!(io::stdout(), "[err]") },
            Ok(o) => write!(io::stdout(), "\x1b[32m{}\x1b[0m", o)
          }.unwrap();
        }
      } else {
        let result = eval_pipeline(command.clone(), &mut environment, !is_final);
  
        match result {
          Err(e) => if is_final {
            linesdown += e.lines().count();
            write!(io::stdout(), "{}", e) 
          } else {
            let towrite = if e.len() > 0 && e.split(" ").nth(0).unwrap() == error::error("", 0, 0, "", "").split(" ").nth(0).unwrap() { "[err]" } else { e.as_str() };
            linesdown += towrite.lines().count();
            write!(io::stdout(), "{}", towrite)
          },
          Ok(v) => {
            if is_final && !command.trim().is_empty() && commit_every {
              replablehistory.push(command.clone());
            }
            let v = format!("{}", v);
            linesdown += v.lines().count();
            write!(io::stdout(), "{}", v)
          }
        }.unwrap();
      }
      
      // back in raw mode
      stdout = match io::stdout().into_raw_mode().map_err(|e| format!("Failed to enter raw mode.  Trace:\n\r{}", e)) {
        Ok(o) => o,
        Err(e) => { println!("\x1b[31m{}\x1b[0m", e); std::process::exit(1); }
      };

      write!(stdout, "{}{}", termion::color::Fg(termion::color::Reset), termion::color::Fg(termion::color::BrightWhite)).unwrap();
      if !is_final {
        write!(stdout, "\r").unwrap();
        if linesdown > 0 { write!(stdout, "{}", termion::cursor::Up(linesdown as u16)).unwrap(); }

        // why right but not down??? absolute black magic
        let right = (pathlen + xc_pos) % (term_cols as usize);
        if right > 0 { write!(stdout, "{}", termion::cursor::Right(right as u16)).unwrap(); }
        
        stdout.flush().unwrap();
      } else {
        write!(stdout, "\n\r").unwrap();
        continue 'termloop;
      }
    }
  }
}
