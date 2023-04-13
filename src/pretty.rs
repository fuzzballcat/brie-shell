use crate::parse::{AST, ASTVal, Location, fathometer};
use crate::charset::{self, CHARSET};
use crate::term;
use crate::token::in_true_operators;

use std::fmt;
use unicode_width::UnicodeWidthStr;
use unicode_segmentation::UnicodeSegmentation;

// stitch together n multiline strings horizontally, with spaces in between
pub fn stitch(ss: Vec<String>, betwixt: &str) -> String {
  let maxheight = ss.iter().map(|s| s.matches("\n").count()).max().unwrap_or(0);
  
  // step 1: pad each
  let padded: Vec<String> = ss.iter().map(|s| {
    let maxlen = s.split("\n").map(|l| l.width()).max().unwrap_or(0);
    let thisheight = s.matches("\n").count();
    s.split("\n").map(|l| format!("{}{}", l, " ".repeat(maxlen - l.width()))).collect::<Vec<String>>().join("\n") + format!("\n{}", " ".repeat(maxlen)).repeat(maxheight - thisheight).as_str()
  }).collect();

  // step 2: stitch
  let mut outstring = String::new();
  for i in 0..maxheight + 1 {
    for (pi, p) in padded.iter().enumerate() {
      outstring += p.split("\n").nth(i).unwrap();
      if pi + 1 < padded.len() { 
        outstring += betwixt;
      }
    }
    
    if i < maxheight {
      outstring += "\n";
    }
  }

  outstring
}

fn do_tree(top: String, lhs: String, rhs: String) -> String {
  let maxwidth = lhs.split("\n").map(|l| l.width()).max().unwrap_or(0) + 1;
  let stitchtogether = stitch(Vec::from([lhs, rhs]), "  ");

  let tlines = top.matches("\n").count() + 1;

  let top = if tlines > 1 {
    let lhs = format!("┌\n{}└", "╎\n".repeat(tlines - 2));
    let rhs = format!("┐\n{}┘", "╎\n".repeat(tlines - 2));

    stitch(Vec::from([lhs, top, rhs]), " ")
  } else {
    top
  };
  
  let mut outstring = String::new();
  outstring += top.as_str();
  outstring += "\n";
  
  let bottomstring = format!("├{}┐\n{}", "─".repeat(maxwidth), stitchtogether);

  if tlines > 1 {
    outstring += stitch(Vec::from([String::new(), bottomstring]), "  ").as_str();
  } else {
    outstring += bottomstring.as_str();
  }
  
  outstring
}

fn do_onetree(top: String, val: String) -> String {
  let mut outstring = String::new();
  outstring += top.as_str();
  outstring += "\n│\n";
  outstring += val.as_str();
  
  outstring
}

impl ASTVal {
  pub fn to_tree(&self) -> String {
    match self {
      ASTVal::Ident(..) | ASTVal::Symbol(..) | ASTVal::Num(..) | ASTVal::String(..) => format!("{}", self),
      ASTVal::Apply(lo, f, ro) => {
        let (lf, rf) = (
          match lo {
            None => "ø".to_string(),
            Some(s) => s.to_tree()
          },
          match ro {
            None => "ø".to_string(),
            Some(s) => s.to_tree()
          }
        );
        do_tree(f.to_tree(), lf, rf)
      },
      ASTVal::Assign(n, v) => {
        format!("{}{}\n{}", n, CHARSET::Assign, v.to_tree())
      },
      ASTVal::AliasAssign(n, v) => {
        format!("{0}{1}{1}\n{2}", n, CHARSET::Assign, v.to_tree())
      },
      ASTVal::Operator(f, o, v) => do_tree(o.to_string(), f.to_tree(), v.to_tree()),
      ASTVal::Command(name, args, stdin, _redir) => {
        do_tree(name.to_string(), ASTVal::Array(args.to_vec()).to_tree(), stdin.to_tree())
      },
      ASTVal::SymbolList(ss) => format!("-{}", ss.join("")),
      ASTVal::Array(vs) => {
        if vs.len() == 0 {
          "()".to_string()
        } else {
          stitch(vs.iter().map(|v| {
            match &v.variant {
              ASTVal::Array(..) => do_onetree("[]".to_string(), v.to_tree()),
              _ => v.to_tree()
            }
          }).collect(), " ")
        }
      }
    }
  }
}

fn truncate_and_dotdotdot(s: String, l: usize) -> String {
  if s.width() > l {
    s.chars().take(l).collect::<String>() + "..."
  } else {
    s
  }
}

// ha
fn truncate_no_dotdotdot(s: String, l: usize) -> String {
  if s.width() > l {
    s.chars().take(l).collect::<String>()
  } else {
    s
  }
}

impl fmt::Display for ASTVal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ASTVal::Command(..) => write!(f, "[PIPE]"),
      ASTVal::Assign(s, v) => write!(f, "{}{}\n\r{}", s, CHARSET::Assign, v.to_tree()),
      ASTVal::AliasAssign(s, v) => write!(f, "{0}{1}{1}\n\r{2}", s, CHARSET::Assign, v.to_tree()),
      ASTVal::Num(i) => write!(f, "{}", i),
      ASTVal::String(s) => {
        let s = s.replace("\t", "  ");
        let s = match s.split("\n").map(|l| truncate_and_dotdotdot(l.to_string(), term::size::<usize>().0.saturating_sub(3 + 4) )).collect::<Vec<String>>() {
          k if k.len() > 15 => k.into_iter().take(15).collect::<Vec<String>>().join("\n") + "\n...",
          k => k.join("\n")
        };
        
        let lines = s.matches("\n").count() + 1;
        let lhs = format!("╭\n{}╰", "│\n".repeat(lines));
        let rhs = format!("╮\n{}╯", "│\n".repeat(lines));
        let avbel = "─".repeat(s.split("\n").map(|l| l.width()).max().unwrap_or(0) + 1);
        let pads = format!("\"{0}\n{1}\n─{0}", avbel, s.split("\n").map(|l| format!(" {} ", l)).collect::<Vec<String>>().join("\n"));

        write!(f, "{}", stitch(Vec::from([lhs, pads, rhs]), ""))
      },
      ASTVal::Symbol(s) => write!(f, "{}", s),
      ASTVal::SymbolList(ss) => {
        write!(f, "-")?;
        for s in ss {
          write!(f, "{}", s.chars().nth(1).unwrap())?;
        }
        Ok(())
      }
      ASTVal::Ident(i) => write!(f, "{}", i),
      ASTVal::Apply(..) => write!(f, "{}", self.to_tree()),
      ASTVal::Array(vs) => {
        let mut inner = vs.into_iter().map(|v| format!("{}", v)).fold(Vec::from([String::new()]), |mut acc, r| {
          if r.matches("\n").count() > 0 {
            if acc[acc.len() - 1].len() == 0 {
              acc.pop();
            }
            acc.push(r);
            acc.push(String::new());
          } else {
            let l = acc.len() - 1;
            if acc[l].len() == 0 {
              acc[l] = r;
            } else {
              acc[l] = stitch(Vec::from([acc[l].clone(), r]), " ");
            }
          }
          acc
        });
        while inner.len() > 0 && inner.last().unwrap().len() == 0 {
          inner.pop();
        }
        let inner = inner.join("\n");
        let inner = inner.split("\n").enumerate().map(|(i, l)| if i == inner.matches("\n").count() { truncate_and_dotdotdot(l.to_string(), term::size::<usize>().0.saturating_sub(3 + 2)) } else { truncate_no_dotdotdot(l.to_string(), term::size::<usize>().0.saturating_sub(3 + 2)) } ).collect::<Vec<String>>().join("\n");

        let arrdepth = format!("{}", fathometer(&AST { variant: ASTVal::Array(vs.to_vec()), location: Location { col: 0, len: 0 } }));
        let depthchars = arrdepth.width();
        
        let (maxwidth, maxheight) = (
          inner.split("\n").map(|l| l.width()).max().unwrap_or(0),
          inner.split("\n").count()
        );

        let extracharsfromdepth = if depthchars > maxwidth { depthchars - maxwidth } else { 0 };
        let maxwidth = std::cmp::max(maxwidth, depthchars);
        
        write!(f, "┌{2}{0}┐\n{1}\n└{3}{0}┘", 
          "─".repeat(maxwidth - depthchars), 
          stitch(
            Vec::from([
              vec!["│"; maxheight].join("\n"), 
              inner, 
              vec![format!("{}│", " ".repeat(extracharsfromdepth)); maxheight].join("\n")
            ]), ""),
          arrdepth,
          "─".repeat(depthchars)
        )
      },
      ASTVal::Operator(..) => write!(f, "{}", self.to_tree()),
    }
  }
}

impl fmt::Display for AST {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(&self.variant, f)
  }
}

impl AST {
  pub fn to_tree(&self) -> String {
    self.variant.to_tree()
  }
}

pub fn highlight(s: &String) -> String {
  let mut out = String::new();

  let mut in_string = false;
  let mut lastchar_wasminus = 0;
  let mut lastchar_waspipe = 0;
  let mut found_comment = false;
  let mut textbuffer = String::new();
  let mut in_string_expecting_escape = false;

  let mut was_color = false;
  for char in (s.to_owned() + " ").graphemes(true) {
    if found_comment {
      out += char;
      continue;
    }
    
    if char == CHARSET::Pipe && lastchar_waspipe > 0 {
      out.insert_str(out.len() - 1, charset::Colors::Comment.to_string().as_str());
      out += char;
      found_comment = true;
      continue;
    }
    lastchar_waspipe = 0;
    
    if char.chars().all(|c| c.is_alphabetic()) || (textbuffer.len() > 0 && char.chars().all(|c| c.is_alphanumeric())) {
      textbuffer += char;
    } else {
      if lastchar_wasminus > 0 && textbuffer.len() > 0 {
        out.insert_str(out.len() - textbuffer.len() - lastchar_wasminus, charset::Colors::Symbol.to_string().as_str());
        was_color = true;
        lastchar_wasminus = 0;
      } else if textbuffer == "pipe" 
             || textbuffer == "collect"
             || textbuffer == "num"
             || textbuffer == "exit"
             || textbuffer == "list"
             || textbuffer == "json"
             || textbuffer == "csv" {
        out.insert_str(out.len() - textbuffer.len(), charset::Colors::BuiltinName.to_string().as_str());
        was_color = true;
      }

      textbuffer.clear();
    }

    if char == "\"" && !in_string_expecting_escape {
      in_string = !in_string;
      if in_string {
        out += charset::Colors::String.to_string().as_str();
      }
      out += char;
      if !in_string {
        was_color = true;
      }
    } else if in_string {
      if !in_string_expecting_escape && char == "\\" {
        out += charset::Colors::Reset.to_string().as_str();
      }
      out += char;
      if in_string_expecting_escape {
        in_string_expecting_escape = false;
        out += charset::Colors::String.to_string().as_str();
      } else if char == "\\" {
        in_string_expecting_escape = true;
      }
    } else if char.chars().all(|c| c.is_numeric()) && textbuffer.len() == 0 {
      out += charset::Colors::Number.to_string().as_str();
      out += char;
      was_color = true;
    } else if in_true_operators(char) || char == CHARSET::Pipe || char == CHARSET::AntiPipe {
      out += charset::Colors::Operator.to_string().as_str();
      out += char;
      was_color = true;
      if char == CHARSET::Pipe {
        lastchar_waspipe += 1;
      }
    }
    
    else {
      if was_color {
        out += charset::Colors::Reset.to_string().as_str();
        was_color = false;
      }
      out += char;
    }

    if char == "-" && !in_string {
      if lastchar_wasminus < 2 { lastchar_wasminus += 1; }
    } else if textbuffer.len() == 0 || in_string {
      lastchar_wasminus = 0;
    }
  }
  
  out + charset::Colors::Reset.to_string().as_str()
}
