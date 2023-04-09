use crate::charset::CHARSET;
use crate::token::{self, Token};
use crate::error::{location_format, noteformat, error};

use std::collections::HashMap;
use std::fmt;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ToRedirect {
  ToStdout,
  ToStderr,
  ToBoth,
  ToNull
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Redirect {
  pub stdout: ToRedirect,
  pub stderr: ToRedirect
}

#[derive(Clone, Debug)]
pub enum ASTVal {
  Assign(String, Box<AST>), // may not nest, via grammar
  AliasAssign(String, Box<AST>), // same
  Num(f32),
  Symbol(String),
  Ident(String),
  String(String),
  Apply(Option<Box<AST>>, Box<AST>, Option<Box<AST>>),
  Array(Vec<AST>),
  Operator(Box<AST>, String, Box<AST>),
  Command(String, Vec<AST>, Box<AST>, Redirect), // name, args, stdin, redir
  SymbolList(Vec<String>) // never seen by eval
}

impl PartialEq for ASTVal {
  fn eq(&self, rhs: &ASTVal) -> bool {
    std::mem::discriminant(self) == std::mem::discriminant(rhs)
  }
}

#[derive(Debug, Clone, Copy)]
pub struct Location {
  pub col: usize,
  pub len: usize
}

#[derive(Clone)]
pub struct AST {
  pub variant: ASTVal,
  pub location: Location
}

// loc not needed
impl fmt::Debug for AST {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self.variant)
  }
}

impl PartialEq for AST {
  fn eq(&self, rhs: &AST) -> bool {
    return self.variant == rhs.variant;
  }
}

pub fn respect_fill(node: &AST, env: &HashMap<String, AST>) -> bool {
  match &node.variant {
    ASTVal::Operator(_, o, _) if o == CHARSET::Reduce.as_str() || o == CHARSET::Scan.as_str() || o == CHARSET::Where.as_str() => false,
    ASTVal::Operator(f, _, _) => respect_fill(f, env),
    ASTVal::Apply(_, _, r) if r.is_some() && is_fn(r.as_ref().unwrap(), env) => respect_fill(&r.as_ref().unwrap(), env),
    ASTVal::Ident(x) => if let Some(id) = env.get(x) { respect_fill(id, env) } else if which::which(x).is_ok() { true } else { false },
    _ => false
  }
}

pub fn fill_from_right(node: &AST, env: &HashMap<String, AST>) -> bool {
  match &node.variant {
    ASTVal::Operator(_, o, _) if o == CHARSET::Reduce.as_str() || o == CHARSET::Scan.as_str() || o == CHARSET::Where.as_str() || o == CHARSET::Iterate.as_str() => true,
    ASTVal::Operator(f, _, _) => fill_from_right(f, env),
    ASTVal::Apply(_, _, r) if r.is_some() && is_fn(r.as_ref().unwrap(), env) => fill_from_right(&r.as_ref().unwrap(), env),
    ASTVal::Apply(l, _, r) => !(r.is_some() && l.is_none()),
    ASTVal::Ident(i) => if let Some(id) = env.get(i) { fill_from_right(id, env) } else { true },
    _ => true
  }
}

pub fn is_fn(node: &AST, env: &HashMap<String, AST>) -> bool {
  stricter_is_fn(node, env) || (match &node.variant {
    ASTVal::Apply(l, _, r) => l.is_none() || r.is_none(),
    _ => false
  })
}

pub fn is_lazy(f: &AST) -> bool {
  match &f.variant {
    ASTVal::Operator(f, _, _) => is_lazy(f),
    ASTVal::Ident(x) if x == "pipe" => true,
    ASTVal::Apply(x, y, z) if
         x.as_ref().and_then(|x| is_lazy(x).then(|| ())).is_some()
      || is_lazy(y)
      || z.as_ref().and_then(|z| is_lazy(z).then(|| ())).is_some()
        => true,
    
    _ => false
  }
}

pub fn stricter_is_fn(node: &AST, env: &HashMap<String, AST>) -> bool {
  (match &node.variant {
    ASTVal::Operator(..) => true,
    ASTVal::Ident(i) => token::in_operators(&i) || if let Some(v) = env.get(i) { is_fn(v, env) } else { which::which(i).is_ok() /* filecommand */ },
    _ => false
  }) || is_train(node, env)
}

fn is_train(node: &AST, env: &HashMap<String, AST>) -> bool {
  match &node.variant {
    ASTVal::Apply(None, f, None) => is_train(f, env),
    ASTVal::Apply(_l, _f, r) => r.as_ref().and_then(|r| is_fn(&*r, env).then(|| ())).is_some(),
    _ => false
  }
}

#[derive(Debug)]
struct ParseRes {
  v: AST,
  is_f: bool
}

fn parse_atom(toks: &mut Vec<Token>, env: &HashMap<String, AST>, s: &String) -> Result<ParseRes, String> {
  if toks.len() == 0 {
    Err(error(s, 0, 0, "Unexpected EOF parsing expression.", "This is an internal error."))
  }
  
  else if toks[0].is_num() {
    let tok = toks.remove(0);
    Ok(ParseRes {
      v: AST { variant: ASTVal::Num(tok.val.parse::<f32>().map_err(|_| error(s, tok.col, tok.val.width(), "Invalid numeric literal.", noteformat!("Found {}.", tok.val)))?), location: Location { col: tok.col, len: tok.val.width()} },
      is_f: tok.followed
   })
  }

  else if toks[0].is_symbol() {
    let tok = toks.remove(0);
    let w = tok.val.width();

    Ok(ParseRes { v: AST { 
      variant: if tok.val.chars().nth(1).unwrap() == '-' {
        ASTVal::Symbol(tok.val.to_string())
      } else {
        let mut res = Vec::new();
        for c in tok.val.chars().skip(1) {
          res.push(format!("-{}", c));
        }

        ASTVal::SymbolList(res)
      },
      location: Location { col: tok.col, len: w }
    }, is_f: tok.followed })
  }

  else if toks[0].is_id(){
    let tok = toks.remove(0);
    let len = tok.val.width();
    Ok(ParseRes { v: AST { variant: ASTVal::Ident(tok.val.to_string()), location: Location { col: tok.col, len } }, is_f: tok.followed })
  }

  else if toks[0].is_string() {
    let tok = toks.remove(0);
    let newchs = tok.val.chars().skip(1);
    
    Ok(ParseRes { v: AST { variant: ASTVal::String(newchs.collect()), location: Location {col: tok.col, len: tok.val.width() } }, is_f: tok.followed })
  }

  else if toks[0].val == "(" {
    let opentok = toks.remove(0);
    let e = parse_command(toks, env, s);
    if toks.len() == 0 {
      Err(error(s, 0, 0, "Expecting close parenthesis.", noteformat!("Open parenthesis found here:\n\r{}", location_format(s, opentok.col, opentok.val.width()))))
    } else if toks[0].val != ")" {
      Err(error(s, toks[0].col, toks[0].val.width(), "Expecting close parenthesis.", noteformat!("Open parenthesis found here:\n\r{}", location_format(s, opentok.col, opentok.val.width()))))
    } else {
      Ok(ParseRes { v: e?, is_f: toks.remove(0).followed })
    }
  }

  else {
    Err(error(s, toks[0].col, toks[0].val.width(), "Expected expression.", "This is an internal error."))
  }
}

fn parse_array(toks: &mut Vec<Token>, env: &HashMap<String, AST>, s: &String) -> Result<Option<ParseRes>, String> {
  let mut exprs = Vec::new();

  let mut anything_at_all = false;
  while token::more_there(toks) {    
    let old_toks = toks.clone();
    let expr = parse_atom(toks, env, s)?;
    
    if stricter_is_fn(&expr.v, env) || (is_fn(&expr.v, env) && expr.is_f) { // backtrack, end of exprlist
      *toks = old_toks;
      break;
    } else {
      // expand symlist
      match expr.v.variant {
        ASTVal::SymbolList(ss) => {
          for (i, s) in ss.iter().enumerate() {
            exprs.push(ParseRes { v: AST { variant: ASTVal::Symbol(s.to_string()), location: Location { col: expr.v.location.col + i, len: 1 }}, is_f: expr.is_f });
          }
        },
        _ => exprs.push(expr)
      };

      anything_at_all = true;
    }
  };

  if !anything_at_all {
    Ok(None)
  } else if exprs.len() == 0 {
    Ok(Some(ParseRes { v: AST { variant: ASTVal::Array(Vec::new()), location: Location { col: 0, len: 0 }}, is_f: false }))
  } else if exprs.len() > 1 {
    let l = exprs[0].v.location;
    let is_f = exprs.last().unwrap().is_f;
    Ok(Some(ParseRes { v: AST { variant: ASTVal::Array(exprs.into_iter().map(|x| x.v).collect()), location: l }, is_f } ))
  } else {
    Ok(Some(exprs.remove(0)))
  }
}

fn parse_operator(toks: &mut Vec<Token>, env: &HashMap<String, AST>, s: &String) -> Result<ParseRes, String> {
  let mut expr = parse_atom(toks, env, s)?;
  
  while toks.len() > 0 && token::in_true_operators(&toks[0].val) {
    let oper = toks.remove(0);
    let len = oper.val.width();
    let old_toks = toks.clone();
    let mut rhs = parse_fcall(toks, env, s)?;

    let lastfollowed;
    if toks.len() < 1 || toks[0].val != CHARSET::EndOperator {
      *toks = old_toks;
      lastfollowed = oper.followed;
      rhs = AST { variant: ASTVal::Array(Vec::new()), location: Location { col: oper.col, len }};
    } else {
      lastfollowed = toks.remove(0).followed;
    }
    
    expr = ParseRes { v: AST { variant: ASTVal::Operator(Box::new(expr.v), oper.val.to_string(), Box::new(rhs)), location: Location { col: oper.col, len } }, is_f: lastfollowed };
  }
  
  Ok(expr)
}

pub fn fathometer(v: &AST) -> i32 {
  match v.variant {
    ASTVal::Array(ref vs) => vs.iter().map(|e| fathometer(e)).max().unwrap_or(-1) + 1,
    _ => 0
  }
}

fn parse_train(toks: &mut Vec<Token>, env: &HashMap<String, AST>, s: &String) -> Result<AST, String> {
  let mut fns = Vec::new();

  let mut free_ride = false;
  while token::more_there(toks) {
    let old_toks = toks.clone();
    
    let newfn = parse_operator(toks, env, s)?;
    let is_f = newfn.is_f;
    let newfn = strip_apply(newfn.v);
    
    if !stricter_is_fn(&newfn, env) && !((is_f || free_ride) && is_fn(&newfn, env)) {
      *toks = old_toks;
      break;
    }
    
    free_ride = is_f;

    fns.push(newfn);
  }

  let trainsides = if fns.len() >= 3 { Some((fns.remove(0), fns.pop().unwrap())) } else { None };
  
  let mut result = fns.pop().unwrap();
  while fns.len() > 0 {
    let f = fns.pop().unwrap();
    let l = f.location;
    result = AST { variant: ASTVal::Apply(None, Box::new(f), Some(Box::new(result))), location: l };
  }

  match trainsides {
    None => {},
    Some((lhs, rhs)) => {
      let l = result.location;
      result = AST { variant: ASTVal::Apply(Some(Box::new(lhs)), Box::new(result), Some(Box::new(rhs))), location: l };
    }
  }

  Ok(result)
}

fn parse_fcall(toks: &mut Vec<Token>, env: &HashMap<String, AST>, s: &String) -> Result<AST, String> {
  let lvals = parse_array(toks, env, s)?.map(|x| x.v);

  if !token::more_there(toks) {
    return Ok(lvals.unwrap_or(AST { variant: ASTVal::Array(Vec::new()), location: Location { col: 0, len: 0 } }));
  }
  
  let fun = parse_train(toks, env, s)?;
  
  let rvals = parse_array(toks, env, s)?.map(|x| x.v);
  
  let l = fun.location;
  Ok(AST { 
    variant: match fun.variant {
      ASTVal::Apply(lb, fb, rb) if rb.as_ref().and_then(|rb| is_fn(&rb, env).then(|| ())).is_none() => ASTVal::Apply(lvals.map(|x| Some(Box::new(x))).unwrap_or(lb), fb, rvals.map(|x| Some(Box::new(x))).unwrap_or(rb)),
      
      _ => ASTVal::Apply(lvals.map(|x| Box::new(x)), Box::new(fun), rvals.map(|x| Box::new(x)))
    },
    location: l
  })
}

fn parse_command(toks: &mut Vec<Token>, env: &HashMap<String, AST>, s: &String) -> Result<AST, String> {
  let mut f = parse_fcall(toks, env, s)?;
  
  while toks.len() > 0 && (toks[0].val == CHARSET::Pipe || toks[0].val == CHARSET::AntiPipe) {
    let pipetok = toks.remove(0);
    let mut f2 = parse_fcall(toks, env, s)?;

    if pipetok.val == CHARSET::AntiPipe {
      (f, f2) = (f2, f); // hehe
    }
    let f2 = strip_apply(f2);

    if !is_fn(&f2, env) {
      let (col, len) = if f2.location.len == 0 { (pipetok.col, pipetok.val.width()) } else { (f2.location.col, f2.location.col) };
      Err(error(s, col, len, "Piping into non-function.", noteformat!("The following occupies a functional position, but is not a function:\n\r{}", f2)))?;
    }
    
    f = AST { variant: ASTVal::Apply(Some(Box::new(f)), Box::new(f2), None), location: Location { col: pipetok.col, len: pipetok.val.width() } };
  }
  
  Ok(f)
}

fn strip_apply(node: AST) -> AST {
  match node.variant {
    ASTVal::Apply(None, f, None) => *f,
    _ => node
  }
}

pub fn parse_commands(toks: &mut Vec<Token>, env: &HashMap<String, AST>, s: &String) -> Result<Option<AST>, String> {
  let mut cmds = None;
  if toks.len() > 0 {
    let cmd = parse_command(toks, env, s);
    match cmd {
      Err(e) => { return Err(e); },
      Ok(n) => {
        if toks.len() > 0 && toks[0].val == CHARSET::Assign {
          if let ASTVal::Ident(i) = strip_apply(n).variant {
            let tok = toks.remove(0);
            match parse_command(toks, env, s) {
              Err(e) => { return Err(e); }
              Ok(value) => cmds = Some(AST { variant: ASTVal::Assign(i, Box::new(value)), location: Location { col: tok.col, len: tok.val.width() } })
            }
          } else {
            return Err(error(s, toks[0].col, toks[0].val.width(), "Identifier must be a valid name.", "A valid name consists of any alphabetic character or an underscore followed by any number of alphanumeric characters or underscores."));
          }
        } else if toks.len() > 0 && toks[0].val == CHARSET::Assign.as_str().repeat(2) {
          if let ASTVal::Ident(i) = strip_apply(n).variant {
            let tok = toks.remove(0);
            match parse_command(toks, env, s) {
              Err(e) => { return Err(e); }
              Ok(value) => cmds = Some(AST { variant: ASTVal::AliasAssign(i, Box::new(value)), location: Location { col: tok.col, len: tok.val.width() } })
            }
          } else {
            return Err(error(s, toks[0].col, toks[0].val.width(), "Identifier must be a valid name.", "A valid name consists of any alphabetic character or an underscore followed by any number of alphanumeric characters or underscores."));
          }
        } else {
          cmds = Some(n);
        }
      }
    };
    
    if toks.len() > 0 {
      return Err(error(s, toks[0].col, toks[0].val.width(), "Expected one command per line.", "Use newline to separate commands."));
    }
  }

  Ok(cmds)
}