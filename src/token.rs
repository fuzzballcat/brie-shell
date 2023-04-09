
use crate::charset::CHARSET;
use crate::error::error;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct Token {
  pub val: String,
  pub col: usize,
  pub followed: bool
}

pub fn in_operators<S: AsRef<str>>(c: S) -> bool {
  let c = c.as_ref();
  
     c == CHARSET::Plus
  || c == CHARSET::Minus
  || c == CHARSET::Times
  || c == CHARSET::Divide
  || c == "cd" 
  || c == "exit"
  || c == "num"
  || c == "collect"
  || c == "pipe"
  || c == CHARSET::LTack
  || c == CHARSET::RTack
  || c == CHARSET::Concat
  || c == CHARSET::Index
  || c == CHARSET::ShapeLength
  || c == CHARSET::Equal
  || c == CHARSET::Iota
  || c == CHARSET::Greater
  || c == CHARSET::Less
  || c == CHARSET::MaxLast
  || c == CHARSET::MinFirst
  || c == CHARSET::Transpose
  || c == CHARSET::Take
  || c == CHARSET::Rotate
  || c == "list"
  || c == "csv"
  || c == "json"
}

// poor naming...
pub fn in_true_operators<S: AsRef<str>>(c: S) -> bool {
  let c = c.as_ref();
  
     c == CHARSET::Each
  || c == CHARSET::Reduce
  || c == CHARSET::Scan
  || c == CHARSET::Where
  || c == CHARSET::Iterate
  || c == CHARSET::Table
  || c == CHARSET::Selfie
}

pub fn tokenize(c: &str) -> Result<Vec<Token>, String> {
  let mut toks = Vec::new();
  let mut iter = c.graphemes(true).enumerate().peekable();
  
  while let Some((col, char)) = iter.next() {
    if char.chars().all(|c| c.is_numeric()) {
      let mut str = String::new();
      str += char;
      while let Some((_, nchar)) = iter.peek() {
        if !nchar.chars().all(|c| c.is_numeric()) && *nchar != "." { break; }
        str += nchar;
        let is_dot = *nchar == ".";
        let olditer = iter.clone();
        iter.next();
        if is_dot && !iter.peek().and_then(|(_, nchar)| nchar.chars().all(|c| c.is_numeric()).then(|| ())).is_some() {
          iter = olditer;
          break;
        }
      }

      toks.push(Token { val: str, col, followed: false });
    } else if char.chars().all(|c| c.is_alphabetic()) || char == "_" {
      let mut str = String::new();
      str += char;

      while let Some((_, nchar)) = iter.peek() {
        if !nchar.chars().all(|c| c.is_alphanumeric()) && *nchar != "_" { break; }
        str += nchar;
        iter.next();
      }

      toks.push(Token { val: str, col, followed: false });
    } else if char == "\"" {
      let mut str = String::new();
      str += char;

      let mut lastcol = col;
      
      loop {
        if let Some((col, nchar)) = iter.peek() {
          lastcol = *col;
          
          let s = *nchar;
          if s == "\\" {
            iter.next();
            if let Some((col, escchar)) = iter.peek() {
              lastcol = *col;
              match *escchar {
                "n" => str.push('\n'),
                "t" => str.push('\t'),
                "\\" => str.push('\\'),
                "\"" => str.push('"'),
                x => { str += s; str += x; }
              };
              iter.next();
            } else {
              break Err(error(c, lastcol + 1, 1, "Unexpected EOF!", "While parsing a string escape code, EOF was reached."));
            }
            continue;
          }

          iter.next();
          
          if s == "\"" { break Ok(()); }
          str += s;
        } else {
          break Err(error(c, lastcol + 1, 1, "Unexpected EOF!", "While parsing a string, EOF was reached.  Expect terminating quote."));
        }
      }?;

      toks.push(Token { val: str, col, followed: false });
    } else if char == "-" && iter.peek().and_then(|x| if x.1.chars().all(|c| c.is_numeric()) { Some(()) } else { None }).is_some() { 
      let mut str = String::new();
      str += char;
      str += iter.next().unwrap().1;

      while let Some((_, nchar)) = iter.peek() {
        if !nchar.chars().all(|c| c.is_numeric()) && *nchar != "." { break; }
        str += nchar;
        iter.next();
      }

      toks.push(Token { val: str, col, followed: false });
    } else if char == "-" && iter.peek().and_then(|x| if x.1.chars().all(|c| c.is_alphabetic()) || x.1 == "-" { Some(()) } else { None }).is_some() {
      let mut str = String::new();
      str += char;
      str += iter.next().unwrap().1;
      
      while let Some((_, nchar)) = iter.peek() {
        if !nchar.chars().all(|c| c.is_alphabetic() || (str.chars().any(|c| c.is_alphabetic()) && c.is_numeric())) && char != "_" { break; }
        str += nchar;
        iter.next();
      }

      if str == "--" {
        // wasting memory is the product of laziness.  and yes, it's one char.
        let (dash, dashy) = (str.pop().unwrap().to_string(), str);
        toks.push(Token { val: dash, col, followed: false });
        toks.push(Token { val: dashy, col, followed: false });
      } else {
        toks.push(Token { val: str, col, followed: false });
      }
    } else if char == CHARSET::Assign && iter.peek().and_then(|(_, x)| (*x == CHARSET::Assign).then(|| ())).is_some() {
      iter.next();
      
      toks.push(Token { val: CHARSET::Assign.as_str().repeat(2), col, followed: false });
    } else if char == CHARSET::Pipe && iter.peek().and_then(|(_, x)| (*x == CHARSET::Pipe).then(|| ())).is_some() {
      break; // comment, exit
    } else if   in_operators(char) 
      || char == "("
      || char == CHARSET::Assign
      || char == CHARSET::Pipe
      || char == CHARSET::AntiPipe
      || char == CHARSET::EndOperator
      || in_true_operators(char) 
    {
      toks.push(Token { val: char.to_string(), col, followed: false });
    } else if char == ")" {
      toks.push(Token { val: char.to_string(), col, followed: false });
    } else if char == " " || char == "\t" {
      while let Some((_, nchar)) = iter.peek() {
        if *nchar != " " || *nchar != "\t" { break }
        iter.next();
      }
      continue;
    } else if char == "\n" {
      break;
    } else {
      return Err(error(c, col, 1, "Unknown token", "This is a typo; this symbol does not exist."));
    }

    if iter.peek().and_then(|(_, c)| (*c != ")" && *c != " " && *c != "\t").then(|| ())).is_some() {
      let l = toks.len();
      toks[l - 1].followed = true;
    }
  }
  
  Ok(toks)
}

impl Token {
  pub fn is_num(&self) -> bool {
      self.val.chars().nth(0).unwrap().is_digit(10)
    || (
         self.val.chars().nth(0).unwrap() == '-' 
      && self.val.chars().nth(1).and_then(|c| c.is_numeric().then(|| ())).is_some()
    ) 
  }
  
  pub fn is_id(&self) -> bool {
    let zch = self.val.chars().nth(0).unwrap();
    zch.is_alphabetic() || zch == '_' || in_operators(&self.val)
  }
  
  pub fn is_symbol(&self) -> bool {
    self.val.chars().count() >= 2 && self.val.chars().nth(0).unwrap() == '-' && !self.val.chars().find(|e| e.is_digit(10)).is_some()
  }
  
  pub fn is_string(&self) -> bool {
    self.val.chars().nth(0).unwrap() == '"'
  }
}

pub fn more_there(toks: &Vec<Token>) -> bool {
  toks.len() > 0 && 
  toks[0].val != ")" && 
  toks[0].val != CHARSET::Assign && 
  toks[0].val != CHARSET::Assign.as_str().repeat(2) &&
  toks[0].val != CHARSET::EndOperator &&
  toks[0].val != CHARSET::Pipe &&
  toks[0].val != CHARSET::AntiPipe &&
  (!in_true_operators(&toks[0].val) || in_operators(&toks[0].val))
}