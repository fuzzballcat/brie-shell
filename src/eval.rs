use crate::error::{error, noteformat, internalfailure};
use crate::charset::CHARSET;
use crate::parse::{AST, ASTVal, Location, ToRedirect, Redirect, is_fn, fill_from_right, is_lazy, fathometer, respect_fill};
use crate::token;

use std::env;
use std::path::Path;
use std::collections::HashMap;
use std::io::Read;
use std::str::FromStr;
use unicode_segmentation::UnicodeSegmentation;
use which::which;

fn arithmetic<S: AsRef<str>>(n: S, l: f32, r: f32) -> f32 {
  let n = n.as_ref();
  
       if n == CHARSET::Plus   { l + r }
  else if n == CHARSET::Minus  { l - r }
  else if n == CHARSET::Times  { l * r }
  else if n == CHARSET::Divide { l / r }
    
  else { unreachable!() }
}

fn is_arithmetic<S: AsRef<str>>(n: S) -> bool {
  let n = n.as_ref();
  
     n == CHARSET::Plus
  || n == CHARSET::Minus
  || n == CHARSET::Times
  || n == CHARSET::Divide
}

fn createcmd<R: Into<std::process::Stdio>, O: Into<std::process::Stdio>, E: Into<std::process::Stdio>, B: Into<std::process::Stdio>>(c: String, ags: Vec<AST>, stdin: R, stdout: O, stdoutcpy: O, stderr: E, stderrcpy: E, both: B, bothcpy: B, redirect: Redirect, s: &String, l: &Location) -> Result<std::process::Child, String> {
  let mut cmd = std::process::Command::new(c);
  cmd.args(ags
      .into_iter()
      .map(|x| match x.variant {
        ASTVal::String(str) | ASTVal::Symbol(str) => Ok(str),
        ASTVal::Num(i) => Ok(i.to_string()),
        ASTVal::Command(..) => {
          let p = inpipe_to_ast(x, s, l, StdoutCaptureType::Data)?;
          match p.variant {
            ASTVal::String(str) | ASTVal::Symbol(str) => Ok(str),
            ASTVal::Num(i) => Ok(i.to_string()),
            _ => Err(error(s, l.col, l.len, "Process requires valid arguments, but was not given one.", noteformat!("The given argument was:\n\r{}", p)))
          }
        },
        _ => Err(error(s, l.col, l.len, "Process requires valid arguments, but was not given one.", noteformat!("The given argument was:\n\r{}", x)))
      })
      .collect::<Result<Vec<_>, _>>()?
    );

  cmd.stdin(stdin);
  match redirect.stdout {
    ToRedirect::ToStdout => cmd.stdout(stdout),
    ToRedirect::ToStderr => cmd.stdout(stderr),
    ToRedirect::ToBoth   => cmd.stdout(both),
    ToRedirect::ToNull   => cmd.stdout(std::process::Stdio::null())
  };
  match redirect.stderr {
    ToRedirect::ToStdout => cmd.stderr(stdoutcpy),
    ToRedirect::ToStderr => cmd.stderr(stderrcpy),
    ToRedirect::ToBoth   => cmd.stderr(bothcpy),
    ToRedirect::ToNull   => cmd.stderr(std::process::Stdio::null())
  };

  let ch = cmd.spawn().map_err(|e| error(s, l.col, l.len, "Failed to spawn command.", noteformat!("Trace:\n\r{}", e)))?; drop(cmd);
  Ok(ch)
}

struct CmdOutput {
  stdout: os_pipe::PipeReader,
  stderrwriter: os_pipe::PipeWriter,
  stderrreader: os_pipe::PipeReader,
  child: std::process::Child
}

fn spawncmd<'a>(c: String, ags: Vec<AST>, stdin: Box<AST>, red: Redirect, scope: &'a std::thread::Scope<'a, '_>, s: &'a String, l: &'a Location) -> Result<CmdOutput, String> {
  let (stdoutreader, mut stdoutwriter) = os_pipe::pipe().map_err(|e| internalfailure(e, "open pipe", s, l))?;
  let (stderrreader, stderrwriter);
  let (mut bothreader, bothwriter) = os_pipe::pipe().map_err(|e| internalfailure(e, "open pipe", s, l))?;
  
  let ch;
  match stdin.variant {
    ASTVal::Command(sc, sags, sstdin, sred) => {
      let CmdOutput { stdout: stdinreader, stderrreader: tempstderrreader, stderrwriter: tempstderrwriter, child: _ch } = spawncmd(sc, sags, sstdin, sred, scope, s, l)?;
      stderrreader = tempstderrreader; // these lines are
      stderrwriter = tempstderrwriter; // quite annoying
      
      ch = createcmd(c, ags, stdinreader,
                     stdoutwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     stdoutwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     stderrwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     stderrwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     bothwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     bothwriter, red, s, l)?;
    },
    _ => {
      (stderrreader, stderrwriter) = os_pipe::pipe().map_err(|e| internalfailure(e, "open pipe", s, l))?;
      
      ch = createcmd(c, ags, std::process::Stdio::inherit(), 
                     stdoutwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     stdoutwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     stderrwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     stderrwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     bothwriter.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?,
                     bothwriter, red, s, l)?;
    }
  }
  
  let mut brcl = bothreader.try_clone().map_err(|e| internalfailure(e, "clone pipe", s, l))?;
  scope.spawn(move || -> Result<(), String> {
    std::io::copy(&mut brcl, &mut stdoutwriter).map_err(|e| internalfailure(e, "copy pipe stream", s, l))?;
    Ok(())
  });
  let mut secl = stderrwriter.try_clone().map_err(|e| error(s, l.col, l.len, "Failed to clone pipe.", noteformat!("Trace:\n\r{}", e)))?;
  scope.spawn(move || -> Result<(), String> {
    std::io::copy(&mut bothreader, &mut secl).map_err(|e| internalfailure(e, "copy pipe stream", s, l))?;
    Ok(())
  });

  Ok(CmdOutput { stdout: stdoutreader, stderrreader, stderrwriter, child: ch })
}

#[derive(Clone, Copy)]
enum StdoutCaptureType {
  None,
  Data,
  All
}

fn inpipe_to_ast(v: AST, s: &String, l: &Location, needs_stdout_capture: StdoutCaptureType) -> Result<AST, String>{
  match v.variant {
    ASTVal::Command(c, ags, stdin, red) => {
      let mut outstring = String::new();
      let mut stderrstring = String::new();
      
      let mut ch = std::thread::scope(|scope| -> Result<std::process::Child, String> {
        let CmdOutput { stdout: mut so, child: ch, stderrreader: mut se, stderrwriter: _sw } = spawncmd(c, ags, stdin, red, scope, s, l)?;

        let ostrref = &mut outstring;
        let errstrref = &mut stderrstring;
        
        match needs_stdout_capture {
          StdoutCaptureType::None => {
            scope.spawn(move || -> Result<(), String> {
              std::io::copy(&mut so, &mut std::io::stdout()).map_err(|e| internalfailure(e, "copy pipe stream", s, l))?;
              Ok(())
            });

            scope.spawn(move || -> Result<(), String> {
              std::io::copy(&mut se, &mut std::io::stderr()).map_err(|e| internalfailure(e, "copy pipe stream", s, l))?;
              Ok(())
            });
          },
          StdoutCaptureType::Data => {
            scope.spawn(move || -> Result<(), String> {
              so.read_to_string(ostrref).map_err(|e| internalfailure(e, "read stdout to string", s, l))?;
              Ok(())
            });

            scope.spawn(move || -> Result<(), String> {
              std::io::copy(&mut se, &mut std::io::stderr()).map_err(|e| internalfailure(e, "copy pipe stream", s, l))?;
              Ok(())
            });
          },
          StdoutCaptureType::All => {
            scope.spawn(move || -> Result<(), String> {
              so.read_to_string(ostrref).map_err(|e| internalfailure(e, "read stdout to string", s, l))?;
              Ok(())
            });

            scope.spawn(move || -> Result<(), String> {
              se.read_to_string(errstrref).map_err(|e| internalfailure(e, "read stderr to string", s, l))?;
              Ok(())
            });
          }
        };

        Ok(ch)
      })?;
  
      let exitcode = ch.wait().map_err(|e| internalfailure(e, "wait for process", s, l))?;

      Ok(match needs_stdout_capture {
        StdoutCaptureType::None => AST { variant: ASTVal::Num(exitcode.code().ok_or(internalfailure("[nil]", "read exit code", s, l))? as f32), location: *l },
        StdoutCaptureType::Data => {
          AST { variant: ASTVal::String(outstring), location: *l }
        },
        StdoutCaptureType::All => {
          AST { variant: ASTVal::Array(Vec::from([
            AST { variant: ASTVal::String(outstring), location: *l },
            AST { variant: ASTVal::Num(exitcode.code().ok_or(internalfailure("[nil]", "read exit code", s, l))? as f32), location: *l },
            AST { variant: ASTVal::String(stderrstring), location: l.clone() }
          ])), location: *l }
        }
      })
    },
    ASTVal::Array(vs) => {
      let mut vsn = Vec::new();
      for v in vs {
        vsn.push(inpipe_to_ast(v, s, l, needs_stdout_capture)?);
      }
      Ok(AST { variant: ASTVal::Array(vsn), location: *l })
    },
    _ => Ok(v)
  }
}

fn unoptionize(node: Option<AST>) -> AST {
  node.unwrap_or(AST { variant: ASTVal::Array(Vec::new()), location: Location { col: 0, len: 0 }})
}

fn nilarr() -> AST {
  AST { variant: ASTVal::Array(Vec::new()), location: Location { col: 0, len: 0 }}
}

fn is_truthy(n: &AST) -> bool {
  if let ASTVal::Num(x) = n.variant { // empty array?
    return x != 0.0;
  } else {
    return true;
  }
}

fn reshape(vals: &Vec<AST>, vind: &mut usize, sizes: &Vec<i32>, depth: usize, location: Location) -> AST {
  if vals.len() < 1 {
    return AST { variant: ASTVal::Array(Vec::new()), location };
  }
  
  let mut result = Vec::new();

  if depth >= sizes.len() {
    let x = vals[*vind].clone();
    *vind += 1; if *vind >= vals.len() { *vind = 0; }
    return x;
  }

  let s = sizes[depth];
  let is_rev = s < 0;
  for _ in 0..s.abs() {
    let v = reshape(vals, vind, sizes, depth + 1, location);
    if is_rev {
      result.insert(0, v);
    } else {
      result.push(v);
    }
  }

  AST { variant: ASTVal::Array(result), location }
}

fn equality(l: &AST, r: &AST) -> bool {
  if std::mem::discriminant(&l.variant) != std::mem::discriminant(&r.variant) {
    return false;
  }

  match &l.variant {
    ASTVal::Assign(i, v) => if let ASTVal::Assign(ref i2, ref v2) = r.variant {
      i == i2 && equality(&v, &v2)
    } else { unreachable!() },
    ASTVal::AliasAssign(i, v) => if let ASTVal::AliasAssign(ref i2, ref v2) = r.variant {
      i == i2 && equality(&v, &v2)
    } else { unreachable!() },
    ASTVal::Num(i) => if let ASTVal::Num(ref i2) = r.variant {
      i == i2
    } else { unreachable!() },
    ASTVal::String(s) => if let ASTVal::String(ref s2) = r.variant {
      s == s2
    } else { unreachable!() },
    ASTVal::Symbol(s) => if let ASTVal::Symbol(ref s2) = r.variant {
      s == s2
    } else { unreachable!() },
    ASTVal::Ident(i) => if let ASTVal::Ident(ref i2) = r.variant {
      i == i2
    } else { unreachable!() },
    ASTVal::Apply(l1, f1, r1) => if let ASTVal::Apply(ref l2, ref f2, ref r2) = r.variant {
      (l1.is_none() && l2.is_none()) || (l1.is_some() && l2.is_some() && equality(&l1.as_ref().unwrap(), &l2.as_ref().unwrap()))
      
      && equality(&f1, &f2) 
      
      && (r1.is_none() && r2.is_none()) || (r1.is_some() && r2.is_some() && equality(&r1.as_ref().unwrap(), &r2.as_ref().unwrap()))
    } else { unreachable!() },
    ASTVal::Operator(f, o, v) => if let ASTVal::Operator(ref f2, ref o2, ref v2) = r.variant {
      equality(&f, &f2) && o == o2 && equality(&v, &v2) 
    } else { unreachable!() },
    ASTVal::SymbolList(ls) => if let ASTVal::SymbolList(ref ls2) = r.variant {
      if ls.len() != ls2.len() {
        return false;
      }
      for i in 0..ls.len() {
        if ls[i] != ls2[i] {
          return false;
        }
      }
      true
    } else { unreachable!() },
    ASTVal::Array(vs) => if let ASTVal::Array(ref vs2) = r.variant {
      if vs.len() != vs2.len() {
        return false;
      }
      for i in 0..vs.len() {
        if !equality(&vs[i], &vs2[i]) {
          return false;
        }
      }
      true
    } else { unreachable!() },
    ASTVal::Command(a, b, c, d) => if let ASTVal::Command(ref a2, ref b2, ref c2, ref d2) = r.variant {
      if b.len() != b2.len() {
        return false;
      }
      for i in 0..b.len() {
        if !equality(&b[i], &b2[i]) {
          return false;
        }
      }
      a == a2 && equality(&c, &c2) && d == d2
    } else { unreachable!() }
  }
}

fn compare(l: &AST, r: &AST) -> std::cmp::Ordering {
  if let (ASTVal::Num(l), ASTVal::Num(r)) = (&l.variant, &r.variant) {
    match () {
      _ if l < r => std::cmp::Ordering::Less,
      _ if l > r => std::cmp::Ordering::Greater,
      _ => std::cmp::Ordering::Equal
    }
  } else if let (ASTVal::String(l), ASTVal::String(r)) = (&l.variant, &r.variant) {
    l.to_lowercase().cmp(&r.to_lowercase())
  } else {
    std::cmp::Ordering::Equal
  }
}

fn minmax(ismax: u8, l: AST, r: AST) -> AST {
  let res = compare(&l, &r);
  let (min, max);
  if res == std::cmp::Ordering::Less {
   (min, max) = (l, r);
  } else if res == std::cmp::Ordering::Greater {
   (min, max) = (r, l);
  } else {
   (min, max) = (l, r);
  }
  
  if ismax > 0 { max } else { min }
}

fn shapeunion(l: Vec<usize>, r: Vec<usize>) -> Vec<usize> {
  let (mut max, min) = if l.len() > r.len() {
    (l, r)
  } else {
    (r, l)
  };

  for i in 0..min.len() {
    if min[i] > max[i] {
      max[i] = min[i];
    }
  }

  max
}

fn shapeof(node: &AST) -> Vec<usize> {
  match &node.variant {
    ASTVal::Array(vs) => {
      let mut l = vs.into_iter().map(|v| shapeof(&v)).reduce(|l, r| shapeunion(l, r)).unwrap_or(Vec::from([0]));
      l.insert(0, vs.len());
      l
    },
    _ => Vec::new()
  }
}

fn set_val<S: AsRef<str>>(arr: &mut AST, mut indices: Vec<usize>, val: AST, s: S) -> Result<(), String> {
  if indices.len() < 1 {
    *arr = val;
    return Ok(());
  }
  
  match &mut arr.variant {
    ASTVal::Array(vs) => { 
      let index = indices.remove(0);
      set_val(&mut vs[index], indices, val, s)
    },
    _ => Err(error(s, arr.location.col, arr.location.len, "Expected an array to index into but found a value.", noteformat!("The value was:\n\r{}", arr)))
  }?;

  Ok(())
}

fn transposify<S: AsRef<str>>(dest: &mut AST, source: AST, axes: &Vec<usize>, mut this_index: Vec<usize>, s: S) -> Result<(), String> {
  match source.variant {
    ASTVal::Array(vs) => {
      this_index.push(0);
      for v in vs {
        transposify(dest, v, axes, this_index.clone(), s.as_ref())?;
        let l = this_index.len() - 1;
        this_index[l] += 1;
      }
    },
    _ => {
      let mut result_index = Vec::new();
      for a in axes {
        if *a >= this_index.len() {
          return Err(error(s, source.location.col, source.location.len, "Malformed shape.", noteformat!("Argument to transpose is malformed for transposition.  The value is:\n\r{}", source)));
        }
        result_index.push(this_index[*a].clone());
      }
      set_val(dest, result_index, source, s)?;
    }
  }
  
  Ok(())
}

fn transpose<S: AsRef<str>>(axes: Option<Vec<usize>>, node: AST, s: S) -> Result<AST, String> {
  let shape = shapeof(&node);

  let axes = match axes {
    Some(axes) => axes,
    None => (0..shape.len()).into_iter().rev().collect()
  };
  
  let reordered_shape = {
    let mut r = Vec::new();
    for a in &axes {
      if *a >= shape.len() {
        return Err(error(s, node.location.col, node.location.len, "Shape out of bounds.", "Transposition axes lie outside the boundaries of the shape of the given argument."));
      }
      r.push(shape[*a]);
    }
    r
  };

  let mut template = Vec::new();
  for _ in 0..reordered_shape.iter().map(|i| *i).reduce(|l, r| l * r).unwrap_or(0) {
    template.push(AST { variant: ASTVal::Num(0.0), location: node.location });
  }
  let mut vind = 0;
  let mut res = reshape(&template, &mut vind, &reordered_shape.iter().map(|i| *i as i32).collect(), 0, node.location);
  
  transposify(&mut res, node, &axes, Vec::new(), s)?;
  
  Ok(res)
}

fn ast_stringify<S: AsRef<str>>(node: AST, s: S, loc: Location) -> Result<String, String> {
  match node.variant {
    ASTVal::String(s) => Ok(format!("{}", s)),
    ASTVal::Symbol(s) => Ok(format!("--{}", s)),
    ASTVal::Ident(i) => Ok(format!("{}", i)),
    ASTVal::Num(n) => Ok(format!("{}", n)),
    
    _ => Err(error(s, loc.col, loc.len, "Invalid item to stringify.", noteformat!("This item must be atomic.  Try using a conversion method first like `list` or `json`.  The given object was:\n\r{}", node)))
  }
}

fn ast_from_jsonvalue(json: json::JsonValue, location: Location) -> AST {
  match json {
    json::JsonValue::Null => AST { variant: ASTVal::Symbol("--Null".to_string()), location },
    json::JsonValue::Short(s) => AST { variant: ASTVal::String(s.into()), location }, json::JsonValue::String(s) => AST { variant: ASTVal::String(s), location },
    json::JsonValue::Number(n) => AST { variant: ASTVal::Num(n.into()), location },
    json::JsonValue::Boolean(b) => AST { variant: ASTVal::Num(b as i32 as f32 * 1.0), location },
    json::JsonValue::Array(a) => AST { variant: ASTVal::Array(a.into_iter().map(|v| ast_from_jsonvalue(v, location)).collect()), location },
    json::JsonValue::Object(_) => {
      let (mut keys, mut vals) = (Vec::new(), Vec::new());
      for (k, v) in json.entries() {
        keys.push(AST { variant: ASTVal::String(k.to_string()), location });
        vals.push(ast_from_jsonvalue(v.clone(), location));
      }

      AST { variant: ASTVal::Array(Vec::from([AST { variant: ASTVal::Array(keys), location }, AST { variant: ASTVal::Array(vals), location }])), location }
    }
  }
}

fn jsonvalue_from_ast<S: AsRef<str>>(ast: AST, s: S) -> Result<json::JsonValue, String> {
  match ast.variant {
    ASTVal::Num(i) => Ok(json::JsonValue::Number(i.into())),
    ASTVal::String(s) | ASTVal::Symbol(s) | ASTVal::Ident(s) => Ok(json::JsonValue::String(s)),
    ASTVal::Array(vs) => Ok(json::JsonValue::Array(vs.into_iter().map(|v| jsonvalue_from_ast(v, s.as_ref())).collect::<Result<Vec<json::JsonValue>, String>>()?)),
    _ => Err(error(s, ast.location.col, ast.location.len, "Invalid AST to jsonify.", noteformat!("The given value was:\n\r{}", ast)))
  }
}

fn scalar_function<S: AsRef<str>>(name: S, larg: Option<AST>, rarg: Option<AST>, s: &String, loc: Location, env: &mut HashMap<String, AST>, fail_extern: bool, redr: Redirect) -> Result<AST, String> {
  let name = name.as_ref();
  
  match name {
    "num" => {
      let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

      match rarg.variant {
        ASTVal::Num(i) => Ok(AST { variant: ASTVal::String(i.to_string()), location: loc }),
        ASTVal::String(sr) => Ok(AST { variant: ASTVal::Num(f32::from_str(sr.trim()).map_err(|_| error(s, loc.col, loc.len, "Invalid candidate for numeric parsing.", noteformat!("The following string was given:\n\r{}", sr)))?), location: loc }),
        _ => Err(error(s, loc.col, loc.len, "Invalid candidate for numeric parsing.", noteformat!("The following value was given:\n\r{}", rarg)))
      }
    },
    
    _ if is_arithmetic(name) => {
      if let (true, Some(ASTVal::Num(r))) = ( larg.is_none() && name == "-", &rarg.as_ref().map(|x| &x.variant)) {
        return Ok(AST { variant: ASTVal::Num(-r), location: loc });
      }

      if let (true, Some(ASTVal::String(r))) = ( larg.is_none() && name == "-", &rarg.as_ref().map(|x| &x.variant)) {
        return Ok(AST { variant: ASTVal::Array(r.graphemes(true).map(|x| AST { variant: ASTVal::String(x.to_string()), location: loc }).collect()), location: loc });
      }

      if let (true, None, Some(x)) = (name == "*", &larg, &rarg) {
        return Ok(AST { variant: ASTVal::Num(!is_truthy(&x) as u8 as f32), location: loc });
      }
      
      let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
      
      if let (ASTVal::Num(l), ASTVal::Num(r)) = (&larg.variant, &rarg.variant) {
        Ok(AST { variant: ASTVal::Num(arithmetic(name, *l, *r)), location: loc })
      } else if let (true, ASTVal::String(s1), ASTVal::String(s2)) = (name == "+", &larg.variant, &rarg.variant) {
        Ok(AST { variant: ASTVal::String(s1.to_string() + s2.as_str()), location: loc })
      } else {
        Err(error(s, loc.col, loc.len, format!("Cannot perform arithmetic {0} on mistyped value.", name), noteformat!("The left value was:\n\r{}\n\rAnd the right value was:\n\r{}", larg, rarg)))
      }
    },

    "cd" => {
      if fail_extern {
        return Err("[cmd]".to_string());
      }

      let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
      
      match rarg.variant {
        ASTVal::String(st) => {
          let root = Path::new(&st);
          if let Err(e) = env::set_current_dir(&root) {
            Err(error(s, loc.col, loc.len, format!("Could not open directory — {}", e), "Try `ls` to list extant directories."))
          } else {
            Ok(AST { variant: ASTVal::Array(Vec::new()), location: loc })
          }
        },
        ASTVal::Array(vs) if vs.len() == 0 => {
          let root = Path::new("..");
          if let Err(e) = env::set_current_dir(&root) {
            Err(error(s, loc.col, loc.len, format!("Could not open directory — {}", e), "This error occurred because a higher directory could not be opened."))
          } else {
            Ok(AST { variant: ASTVal::Array(Vec::new()), location: loc })
          }
        }
        _ => Err(error(s, rarg.location.col, rarg.location.len, "Expected string to indicate directory.", noteformat!("The given value was:\n\r{}", rarg)))
      }
    },

    "exit" => {
      if fail_extern {
        return Err("[cmd]".to_string());
      }

      let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
      
      match rarg.variant {
        ASTVal::Num(n) if n.fract() == 0.0 => std::process::exit(n.round() as i32),
        ASTVal::Array(vs) if vs.len() == 0 => std::process::exit(0),
        _ => Err(error(s, rarg.location.col, rarg.location.len, "Require number for exit code.", noteformat!("The given value was:\n\r{}", rarg)))
      }
    },

    "list" => {
      let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

      match rarg.variant {
        ASTVal::String(s) => {
          let mut vs = s.split("\n").map(|e| AST { variant: 
              match e.split("\t").collect::<Vec<&str>>() {
                es if es.len() != 1 => ASTVal::Array(es.into_iter().map(|e| AST { variant: ASTVal::String(e.to_string()), location: loc }).collect()),
                mut es => ASTVal::String(es.remove(0).to_string())
              }, location: loc }).collect::<Vec<AST>>();

          if vs.len() > 0 && match &vs[vs.len() - 1].variant { ASTVal::String(s) if s.len() == 0 => true, _ => false } {
            vs.pop();
          }
          
          Ok(AST { variant: match vs { vs if vs.len() != 1 => ASTVal::Array(vs), mut vs => vs.remove(0).variant }, location: loc })
        },
        ASTVal::Array(vs) => Ok(AST { variant: ASTVal::String(vs.into_iter().map(|e| match e.variant {
          ASTVal::Array(vs2) => vs2.into_iter().map(|e| ast_stringify(e, s, loc)).collect::<Result<Vec<String>, String>>().map(|x| x.join("\t")),
          _ => ast_stringify(e, s, loc)
        }).collect::<Result<Vec<String>, String>>()?.join("\n")), location: loc }),
        _ => Err(error(s, loc.col, loc.len, "Invalid argument to list.", "List either requires a string to translate to an array or an array to translate to a string."))
      }
    },
    "csv" => {
      let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

      let delimiter = match larg.variant {
        ASTVal::String(s) => s.chars().nth(0).unwrap_or(','),
        _ => ','
      } as u8;

      match rarg.variant {
        ASTVal::String(st) => {
          let mut rdr = csv::ReaderBuilder::new().flexible(true).delimiter(delimiter).from_reader(st.as_bytes());
          
          let mut records = rdr.records().collect::<Result<Vec<csv::StringRecord>, csv::Error>>().map_err(|_e| error(s, loc.col, loc.len, "Failed to parse CSV.", noteformat!("The given value was:\n\r{}", st)))?;

          records.insert(0, rdr.headers().map_err(|_e| error(s, loc.col, loc.len, "Failed to parse CSV headers.", noteformat!("The given value was:\n\r{}", st)))?.clone());
          
          let records = records.into_iter().map(|r| r.deserialize(None)).collect::<Result<Vec<Vec<String>>, csv::Error>>().map_err(|_e| error(s, loc.col, loc.len, "Failed to deserialize CSV object.", noteformat!("The given value was:\n\r{}", st)))?;

          Ok(AST { variant: ASTVal::Array(records.into_iter().map(|r| AST { variant: ASTVal::Array(r.into_iter().map(|sss| AST { variant: ASTVal::String(sss), location: loc }).collect()), location: loc }).collect()), location: loc })
        },
        ASTVal::Array(vs) => {
          let mut wtr = csv::WriterBuilder::new().flexible(true).delimiter(delimiter).from_writer(Vec::new());

          for v in vs {
            let mut vc = Vec::new();
            match v.variant {
              ASTVal::Array(vs2) => {
                for v2 in vs2 {
                  vc.push(ast_stringify(v2, s, loc)?);
                }
              },
              _ => {
                vc.push(ast_stringify(v, s, loc)?);
              }
            };

            wtr.write_record(vc).map_err(|e| internalfailure(e, "write to CSV record", s, &loc))?;
          }
          Ok(AST { variant: ASTVal::String(String::from_utf8(wtr.into_inner().map_err(|e| internalfailure(e, "unwrap CSV record object", s, &loc))?).map_err(|e| internalfailure(e, "convert CSV object to string", s, &loc))?), location: loc })
        },
        _ => Err(error(s, loc.col, loc.len, "Invalid argument to csv.", "CSV either requires a string to translate to an array or an array to translate to a string."))
      }
    },
    "json" => {
      let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
      match rarg.variant {
        ASTVal::String(st) => {
          let parsed = json::parse(&st).map_err(|_e| error(s, loc.col, loc.len, "Failed to parse JSON.", noteformat!("Invalid JSON:\n\r{}", st)))?;
          Ok(ast_from_jsonvalue(parsed, loc))
        },
        ASTVal::Array(_) => {
          let converted = jsonvalue_from_ast(rarg, s)?;
          Ok(AST { variant: ASTVal::String(json::stringify(converted)), location: loc })
        },
        _ => Err(error(s, loc.col, loc.len, "Invalid argument to json.", "JSON either requires a string to translate to an array or an array to translate to a string."))
      }
    },

    x if x == CHARSET::LTack => {
      Ok(unoptionize(larg))
    },
    x if x == CHARSET::RTack => {
      Ok(unoptionize(rarg))
    },
    x if x == CHARSET::Rotate => {
      if larg.is_some() {
        let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

        let rotate = match larg.variant {
          ASTVal::Num(n) if n.fract() == 0.0 => Ok(n as i32),
          _ => Err(error(s, loc.col, loc.len, "Invalid rotation degree.", noteformat!("Rotate requires an integral argument to rotate by, instead found:\n\r{}", larg)))
        }?;

        let elems = match rarg.variant {
          ASTVal::Array(vs) => vs,
          _ => Vec::from([rarg])
        };
        
        let mut res = Vec::new();
        let len = elems.len();
        for i in 0..len {
          res.push(elems[(i as i32 + rotate).rem_euclid(len as i32) as usize].clone());
        }
        Ok(AST { variant: ASTVal::Array(res), location: loc })
      } else {
        let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

        Ok(AST { variant: match rarg.variant {
          ASTVal::Array(vs) => ASTVal::Array(vs.into_iter().rev().collect()),
          x => x
        }, location: loc })
      }
    },
    x if x == CHARSET::Take => {
      let (larg, rarg) = (match larg { Some(larg) => inpipe_to_ast(larg, s, &loc, StdoutCaptureType::Data)?, None => AST { variant: ASTVal::Num(1.0), location: loc } }, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

      let takenum = match larg.variant {
        ASTVal::Num(i) if i.fract() == 0.0 => Ok(i as i32),
        _ => Err(error(s, loc.col, loc.len, "Invalid number to take.", noteformat!("Expected integral count to take, instead found:\n\r{}", larg)))
      }?;

      match rarg.variant {
        ASTVal::Array(mut vs) => {
          if takenum.abs() as usize > vs.len() {
            return Err(error(s, loc.col, loc.len, "Index out of bounds error.", noteformat!("The given count to take is greater than the length of the array.  The count was:\n\r{}\n\rBut the array was:\n\r{}", takenum, AST { variant: ASTVal::Array(vs), location: loc })));
          }
          if takenum >= 0 {
            let takenum = takenum as usize;
            Ok(AST { variant: ASTVal::Array(vs.drain(0..takenum).collect()), location: loc })
          } else {
            let takenum = -takenum as usize;
            let (s, e) = (vs.len() - takenum, vs.len());
            Ok(AST { variant: ASTVal::Array(vs.drain(s..e).collect()), location: loc })
          }
        },
        _ => Ok(rarg)
      }
    },
    x if x == CHARSET::Transpose => {
      let (larg, rarg) = (match larg { Some(larg) => Some(arrayifyast(inpipe_to_ast(larg, s, &loc, StdoutCaptureType::Data)?)), None => None }, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

      let ls = larg.map(|larg| match larg.variant {
        ASTVal::Array(vs) => vs,
        _ => unreachable!()
      });

      let ls = match ls {
        Some(ls) => Some(ls.into_iter().map(|l| match l.variant {
        ASTVal::Num(x) if x >= 0.0 && x.fract() == 0.0 => Ok(x as u32 as usize),
        _ => Err(error(s, l.location.col, l.location.len, "Invalid axis specifier.", noteformat!("Expected an integral positive numeric argument, but instead found:\n\r{}", l)))
      }).collect::<Result<Vec<usize>, String>>()?),
        None => None
      };

      transpose(ls, rarg, s)
    },
    x if x == CHARSET::MaxLast => {
      if larg.is_some() {
        let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
        Ok(minmax(1, larg, rarg))
      } else {
        let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

        Ok(match arrayifyast(rarg).variant {
          ASTVal::Array(mut vs) if vs.len() > 0 => vs.pop().unwrap(),
          _ => AST { variant: ASTVal::Array(Vec::new()), location: loc }
        })
      }
    },
    x if x == CHARSET::MinFirst => {
      if larg.is_some() {
        let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
        Ok(minmax(0, larg, rarg))
      } else {
        let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
          
        Ok(match arrayifyast(rarg).variant {
          ASTVal::Array(mut vs) if vs.len() > 0 => vs.remove(0),
          _ => AST { variant: ASTVal::Array(Vec::new()), location: loc }
        })
      }
    },
    x if x == CHARSET::Concat => {
      let (larg, rarg) = (if larg.is_some() { Some(inpipe_to_ast(larg.unwrap(), s, &loc, StdoutCaptureType::Data)?) } else { None }, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
      match larg {
        None => {
          Ok(AST { variant: ASTVal::Array(Vec::from([rarg])), location: loc })
        },
        Some(larg) => {
          let mut larg = match arrayifyast(larg).variant {
            ASTVal::Array(vs) => vs,
            _ => unreachable!()
          };
          let rarg = match arrayifyast(rarg).variant {
            ASTVal::Array(vs) => vs,
            _ => unreachable!()
          };
    
          larg.extend(rarg);
          Ok(AST { variant: ASTVal::Array(larg), location: loc })
        }
      }
    },
    x if x == CHARSET::Greater => {
      if larg.is_some() {
        let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
        
        Ok(AST { variant: ASTVal::Num((compare(&larg, &rarg) == std::cmp::Ordering::Less) as u32 as f32), location: loc })
      } else {
        let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, arrayifyast(inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?));

        let vs = match rarg.variant {
          ASTVal::Array(vs) => vs,
          _ => unreachable!()
        };
        let mut is: Vec<usize> = (0..vs.len()).collect();
        is.sort_by(|a, b| compare(&vs[*a], &vs[*b]));
        
        Ok(AST { variant: ASTVal::Array(is.into_iter().map(|i| AST { variant: ASTVal::Num(i as f32), location: loc }).collect()), location: loc })
      }
    },
    x if x == CHARSET::Less => {
      if larg.is_some() {
        let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
        
        Ok(AST { variant: ASTVal::Num((compare(&larg, &rarg) == std::cmp::Ordering::Greater) as u32 as f32), location: loc })
      } else {
        let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, arrayifyast(inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?));

        let vs = match rarg.variant {
          ASTVal::Array(vs) => vs,
          _ => unreachable!()
        };
        let mut is: Vec<usize> = (0..vs.len()).collect();
        is.sort_by(|a, b| compare(&vs[*a], &vs[*b]).reverse());
        
        Ok(AST { variant: ASTVal::Array(is.into_iter().map(|i| AST { variant: ASTVal::Num(i as f32), location: loc }).collect()), location: loc })
      }
    },
    
    x if x == CHARSET::Index => {
      let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
      
      match rarg.variant {
        ASTVal::Array(ref vs) => {
          match larg.variant {
            ASTVal::Num(i) => if i >= 0.0 && i.fract() == 0.0 {
              let iusize = i as usize;
              if vs.len() > iusize {
                Ok(vs[iusize].clone())
              } else {
                Err(error(s, loc.col, loc.len, "Index out of bounds.", noteformat!("The index {} is out of bounds of:\n\r{}", iusize, rarg)))
              }
            } else {
              Err(error(s, loc.col, loc.len, "Index must be a nonnegative integer.", noteformat!("The index supplied was: {}", larg)))
            },
            _ => Err(error(s, loc.col, loc.len, "Expected numeric index.", noteformat!("The index supplied was:\n\r{}", larg)))
          }
        },
        _ => Err(error(s, loc.col, loc.len, "Expected array to index into.", noteformat!("The value supplied was instead:\n\r{}", rarg)))
      }
    },

    x if x == CHARSET::ShapeLength => {
      if larg.is_none() {
        let rarg = inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?;
        
        Ok(AST { variant: ASTVal::Array(shapeof(&rarg).into_iter().map(|n| AST { variant: ASTVal::Num(n as u32 as i32 as f32), location: loc }).collect()), location: loc })
      } else {
        let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

        let vs = match arrayifyast(larg).variant {
          ASTVal::Array(vs) => vs,
          _ => unreachable!()
        };

        let mut vs = vs.into_iter().map(|v| match v.variant {
          ASTVal::Num(i) if i.fract() == 0.0 => Ok(i as i32),
          _ => Err("bad num".to_string())
        }).collect::<Result<Vec<i32>, String>>()?;

        let mut rs = match arrayifyast(rarg).variant {
          ASTVal::Array(vs) => if vs.len() < 1 {
            Vec::from([AST { variant: ASTVal::Num(0.0), location: loc }])
          } else {
            vs 
          },
          _ => unreachable!()
        };

        let mut vind = 0;
        Ok(reshape(&mut rs, &mut vind, &mut vs, 0, loc))
      }
    },

    x if x == CHARSET::Equal => {
      let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

      Ok(AST { variant: ASTVal::Num(equality(&larg, &rarg) as u8 as f32), location: loc })
    },

    x if x == CHARSET::Iota => {
      if larg.is_some() {
        let (larg, rarg) = (arrayifyast(inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?), arrayifyast(inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?));

        let mut res = Vec::from([Vec::new()]);
        if let (ASTVal::Array(ls), ASTVal::Array(mut rs)) = (larg.variant, rarg.variant) {
          for i in 0..rs.len() {
            if i >= ls.len() { break; }

            let v = rs.remove(0);
            
            if is_truthy(&ls[i]) {
              res.last_mut().unwrap().push(v);
            } else {
              res.push(Vec::new());
            }
          }
        } else {
          unreachable!();
        }

        return Ok(AST { variant: ASTVal::Array(res.into_iter().map(|x| AST { variant: ASTVal::Array(x), location: loc }).collect()), location: loc });
      }
      
      let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

      let mut result = Vec::new();
      match rarg.variant {
        ASTVal::Num(i) => {
          let mut n = 0.0;
          while n < i {
            result.push(AST { variant: ASTVal::Num(n), location: loc });
            n += 1.0;
          }
        },
        _ => Err(error(s, rarg.location.col, rarg.location.len, "Invalid argument to iota.", noteformat!("Iota expects a numeric argument for sequence length, but instead found:\n\r{}", rarg)))?
      }

      Ok(AST { variant: ASTVal::Array(result), location: loc })
    },

    "collect" => {
      let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::All)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::All)?);

      Ok(rarg)
    },

    "pipe" => {
      let rarg = inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?;
      
      let mut redir = Redirect { 
        stdout: ToRedirect::ToStdout,
        stderr: ToRedirect::ToStderr
      };

      let mut is_unchanged = (true, true);

      let cannederr = Err(error(s, rarg.location.col, rarg.location.len, "Invalid arguments to ].", noteformat!("] expects pairs of symbols indicating rerouting.  It recieved:\n\r{}", rarg)));
      
      let arrai = arrayifyast(rarg);
      match arrai.variant {
        ASTVal::Array(vs) => {
          let mut vs = vs;
          for i in 0..vs.len() {
            match &vs[i].variant {
              ASTVal::Symbol(x) if x == "--swap" => {
                vs.splice(i..i+1, 
                  [
                    AST { variant: ASTVal::Symbol("-o".to_string()), location: vs[i].location },
                    AST { variant: ASTVal::Symbol("-e".to_string()), location: vs[i].location },
                    AST { variant: ASTVal::Symbol("-e".to_string()), location: vs[i].location },
                    AST { variant: ASTVal::Symbol("-o".to_string()), location: vs[i].location }
                  ]
               );
              },
              ASTVal::Symbol(x) if x == "--null" => {
                vs.splice(i..i+1, 
                  [
                    AST { variant: ASTVal::Symbol("-o".to_string()), location: vs[i].location },
                    AST { variant: ASTVal::Symbol("-n".to_string()), location: vs[i].location },
                    AST { variant: ASTVal::Symbol("-e".to_string()), location: vs[i].location },
                    AST { variant: ASTVal::Symbol("-n".to_string()), location: vs[i].location }
                  ]
               );
              }
              _ => {}  
            }
          }
          
          if vs.len() % 2 != 0 {
            return cannederr;
          }

          for pair in vs.chunks(2) {
            let lsym = match &pair[0].variant {
              ASTVal::Symbol(s) => s,
              _ => { return cannederr; }
            };
            let rsym = match &pair[1].variant {
              ASTVal::Symbol(s) => s,
              _ => { return cannederr; }
            };

            let togo = match rsym.as_str() {
              "-o" => ToRedirect::ToStdout,
              "-e" => ToRedirect::ToStderr,
              "-n" => ToRedirect::ToNull,
              _ => { return cannederr; }
            };
            
            match lsym.as_str() {
              "-o" => {
                if is_unchanged.0 || redir.stdout == ToRedirect::ToNull {
                  redir.stdout = togo;
                } else {
                  if togo != ToRedirect::ToNull && togo != redir.stdout {
                    redir.stdout = ToRedirect::ToBoth;
                  }
                }

                is_unchanged.0 = false;
              },
              "-e" => {
                if is_unchanged.1 || redir.stderr == ToRedirect::ToNull {
                  redir.stderr = togo;
                } else {
                  if togo != ToRedirect::ToNull && togo != redir.stderr {
                    redir.stderr = ToRedirect::ToBoth;
                  }
                }

                is_unchanged.1 = false;
              }
              _ => { return cannederr; }
            };
          }
        },
        _ => unreachable!()
      };

      let larg = eval_command(unoptionize(larg), s, env, fail_extern)?;
      match larg.variant {
        ASTVal::Command(c, ags, si, _r) => Ok(AST { variant: ASTVal::Command(c, ags, si, redir), location: larg.location }),
        _ => Ok(larg)
      }
    }
    
    comm => {
      if fail_extern {
        return Err("[cmd]".to_string());
      }

      let rr = unoptionize(rarg);
      
      Ok(AST{ 
        variant: ASTVal::Command(
          comm.to_string(),
          match rr.variant {
            ASTVal::Array(vs) => vs,
            _ => Vec::from([rr])
          },
          Box::new(unoptionize(larg)),
          redr
        ),
        location: loc 
      })
    }
  }
}

fn ranked_fncall(fun: AST, larg: Option<AST>, rarg: Option<AST>, s: &String, lrank: i32, rrank: i32, env: &mut HashMap<String, AST>, fail_extern: bool, redr: Redirect) -> Result<AST, String> {
  let loc = fun.location;
  
  let lrank = if lrank < 0 { fathometer(larg.as_ref().unwrap_or(&nilarr())) - lrank - 1 } else { lrank };
  let rrank = if rrank < 0 { fathometer(rarg.as_ref().unwrap_or(&nilarr())) - rrank - 1 } else { rrank };

  if lrank == 0 && rrank == 0 {
    match fun.variant {
      ASTVal::Ident(ref x) => scalar_function(&x, larg, rarg, s, loc, env, fail_extern, redr),
      _ => call_function(larg, fun, rarg, s, fail_extern, redr, env)
    }
  } else if lrank == 0 {
    let naerr = Err(error(s, loc.col, loc.len, "Attempting to apply rankwise to nonarray.", noteformat!("The right rank necessitated an array, but there was instead:\n\r{}", rarg.as_ref().unwrap_or(&nilarr()))));
    
    if let Some(ASTVal::Array(vs)) = rarg.map(|x| x.variant) {
      let mut res = Vec::new();
      for v in vs {
        res.push(ranked_fncall(fun.clone(), larg.clone(), Some(v), s, lrank, rrank - 1, env, fail_extern, redr)?);
      }
      Ok(AST { variant: ASTVal::Array(res), location: loc })
    } else {
      naerr
    }
  } else if rrank == 0 {
    let naerr = Err(error(s, loc.col, loc.len, "Attempting to apply rankwise to nonarray.", noteformat!("The left rank necessitated an array, but there was instead:\n\r{}", larg.as_ref().unwrap_or(&nilarr()))));
    if let Some(ASTVal::Array(vs)) = larg.map(|x| x.variant) {
      let mut res = Vec::new();
      for v in vs {
        res.push(ranked_fncall(fun.clone(), Some(v), rarg.clone(), s, lrank - 1, rrank, env, fail_extern, redr)?);
      }
      Ok(AST { variant: ASTVal::Array(res), location: loc })
    } else {
      naerr
    }
  } else {
    let lengtherror = Err(error(s, loc.col, loc.len, "Length mismatch.", noteformat!("While applying rankwise, the left hand side was:\n\r{}\n\rBut the right hand side was:\n\r{}", larg.as_ref().unwrap_or(&nilarr()), rarg.as_ref().unwrap_or(&nilarr()))));
    let arrerror = Err(error(s, loc.col, loc.len, "Attempting to apply rankwise to nonarray.", noteformat!("The left and right ranks necessitated arrays, but the left hand side was:\n\r{}\n\rAnd the right hand side was:\n\r{}", larg.as_ref().unwrap_or(&nilarr()), rarg.as_ref().unwrap_or(&nilarr()))));

    let (lvar, rvar) = (larg.map(|x| x.variant), rarg.map(|x| x.variant));
    
    match lvar {
      Some(ASTVal::Array(ls)) => {
        if let Some(ASTVal::Array(rs)) = rvar {
          if ls.len() != rs.len() {
            lengtherror
          } else {
            let mut res = Vec::new();
            for (l, r) in ls.into_iter().zip(rs) {
              res.push(ranked_fncall(fun.clone(), Some(l), Some(r), s, lrank - 1, rrank - 1, env, fail_extern, redr)?);
            }
            Ok(AST { variant: ASTVal::Array(res), location: loc })
          }
        } else {
          arrerror
        }
      },
      None => {
        if let Some(ASTVal::Array(rs)) = rvar {
          let mut res = Vec::new();
          for r in rs {
            res.push(ranked_fncall(fun.clone(), None, Some(r), s, lrank - 1, rrank - 1, env, fail_extern, redr)?);
          }
          Ok(AST { variant: ASTVal::Array(res), location: loc })
        } else {
          arrerror
        }
      },
      _ => arrerror
    }
  }
}

fn is_command<S: AsRef<str>>(f: S) -> bool {
  !token::in_operators(f)
}

fn rankof_idfn<S: AsRef<str>>(f: S, isdyad: bool) -> (i32, i32) {
  let f = f.as_ref();
  
  if is_command(f) || f == "cd" || f == "exit" || f == "pipe" || f == "list" || f == "csv" || f == "json" {
    (-1, 0)
  } else if f == CHARSET::ShapeLength || f == CHARSET::Concat || f == CHARSET::Transpose {
    (0, 0)
  } else if f == CHARSET::RTack || f == CHARSET::LTack {
    (0, 0)
  } else if f == CHARSET::Index || f == CHARSET::Take || f == CHARSET::Rotate {
    (-1, 0)
  } else if isdyad && f == CHARSET::Iota {
    (0, 0)
  } else if !isdyad && (f == CHARSET::Greater || f == CHARSET::Less || f == CHARSET::MinFirst || f == CHARSET::MaxLast) {
    (0, 0)
  } else {
    (-1, -1)
  }
}

fn arrayifyast(v: AST) -> AST {
  let l = v.location;
  match v.variant {
    ASTVal::Array(..) => v,
    _ => {      
      AST { variant: ASTVal::Array(Vec::from([v])), location: l }
    }
  }
}

#[derive(Debug)]
enum NumericMatrix {
  Num(usize),
  Vector(Vec<NumericMatrix>)
}

fn ranked_fixpoint(f: AST, larg: Option<AST>, rarg: AST, times: NumericMatrix, s: &String, fail_extern: bool, redr: Redirect, env: &mut HashMap<String, AST>) -> Result<AST, String> {
  match times {
    NumericMatrix::Num(times) => {
      let mut result = rarg;
      for _ in 0..times {
        result = call_function(larg.clone(), f.clone(), Some(result), s, fail_extern, redr, env)?;
      }
    
      Ok(result)
    },
    NumericMatrix::Vector(times) => {
      let mut res = Vec::new();
      for time in times {
        res.push(ranked_fixpoint(f.clone(), larg.clone(), rarg.clone(), time, s, fail_extern, redr, env)?);
      }
      Ok(AST { variant: ASTVal::Array(res), location: rarg.location })
    }
  }
}

fn fixpoint(f: AST, larg: Option<AST>, rarg: AST, times: NumericMatrix, is_fixpoint: bool, s: &String, fail_extern: bool, redr: Redirect, env: &mut HashMap<String, AST>) -> Result<AST, String> {
  if is_fixpoint {
    let mut result = rarg;
    let mut tries = 0;
    loop {
      let past = result.clone();
      
      result = call_function(larg.clone(), f.clone(), Some(result), s, fail_extern, redr, env)?; 

      if equality(&past, &result) {
        break;
      }

      tries += 1;
      if tries > 32768 {
        let l = result.location;
        return Err(error(s, l.col, l.len, "Failed to find fixpoint.", "After iterating 2^15 times, no fixpoint was found."));
      }
    }

    return Ok(result);
  }
  
  return ranked_fixpoint(f, larg, rarg, times, s, fail_extern, redr, env)
}

fn numerify_vector<S: AsRef<str>>(v: AST, s: S) -> Result<NumericMatrix, String> {
  let s = s.as_ref();
  match v.variant {
    ASTVal::Num(i) if i > 0.0 && i.fract() == 0.0 => Ok(NumericMatrix::Num(i as u64 as usize)),
    ASTVal::Array(vs) => {
      let mut res = Vec::new();
      for v in vs {
        res.push(numerify_vector(v, s)?);
      }
      Ok(NumericMatrix::Vector(res))
    }
    _ => Err(error(s, v.location.col, v.location.len, "Invalid argument to iterate.", "Iterate expects an integral argument, a [possibly nested] array of such arguments, or a function which returns such an argument."))?
  }
}

fn call_function(larg: Option<AST>, fun: AST, rarg: Option<AST>, s: &String, fail_extern: bool, redr: Redirect, env: &mut HashMap<String, AST>) -> Result<AST, String> {
  let (larg, rarg) = match respect_fill(&fun, env) {
    false => match fill_from_right(&fun, env) {
      true => if rarg.is_none() { (None, larg) } else { (larg, rarg) },
      false => if larg.is_none() { (rarg, None) } else { (larg, rarg) }
    },
    true => (larg, rarg)
  };
  
  match fun.variant {
    ASTVal::Ident(ref x) => {
      let lrank = rankof_idfn(&x, larg.is_some() && rarg.is_some()).0;
      let rrank = rankof_idfn(&x, larg.is_some() && rarg.is_some()).1;
      
      ranked_fncall(fun, larg, rarg, s, lrank, rrank, env, fail_extern, redr)
    },

    ASTVal::Operator(f, o, v) => {
      let v = if is_fn(&v, env) {
        Box::new(call_function(larg.clone(), *v, rarg.clone(), s, fail_extern, redr, env)?)
      } else { v };
      match o.as_str() {
        x if x == CHARSET::Selfie => {
          let loc = f.location;
          if larg.is_some() {
            let (larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);
            call_function(Some(rarg), *f, Some(larg), s, fail_extern, redr, env)
          } else {
            let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::Data)?, inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::Data)?);

            call_function(Some(rarg.clone()), *f, Some(rarg), s, fail_extern, redr, env)
          }
        },
        x if x == CHARSET::Table => {
          let loc = f.location;
          let (ls, rs) = (
            match arrayifyast(inpipe_to_ast(unoptionize(larg), s, &loc, StdoutCaptureType::All)?).variant {
              ASTVal::Array(vs) => vs,
              _ => unreachable!()
            },
            match arrayifyast(inpipe_to_ast(unoptionize(rarg), s, &loc, StdoutCaptureType::All)?).variant {
              ASTVal::Array(vs) => vs,
              _ => unreachable!()
            }
          );

          let mut res = Vec::new();
          for l in ls {
            res.push(Vec::new());
            for r in &rs {
              res.last_mut().unwrap().push(
                call_function(Some(l.clone()), *f.clone(), Some(r.clone()), s, fail_extern, redr, env)?
              );
            }
          }

          Ok(AST { variant: ASTVal::Array(res.into_iter().map(|r| AST { variant: ASTVal::Array(r), location: loc }).collect()), location: loc })
        },
        x if x == CHARSET::Iterate => {
          let times = numerify_vector(*v, s)?;
          let is_fixpoint = match times { NumericMatrix::Vector(ref v) if v.len() == 0 => true, _ => false };
          
          let (larg, rarg) = (larg.map(|l| inpipe_to_ast(l, s, &f.location, StdoutCaptureType::Data)), inpipe_to_ast(unoptionize(rarg), s, &f.location, StdoutCaptureType::Data)?);
          let larg = if let Some(Err(e)) = larg {
            return Err(e);
          } else {
            larg.map(|l| l.unwrap())
          };

          fixpoint(*f, larg, rarg, times, is_fixpoint, s, fail_extern, redr, env)
        },
        x if x == CHARSET::Where => {
          let mut res = Vec::new();
          let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &f.location, StdoutCaptureType::Data)?, arrayifyast(inpipe_to_ast(unoptionize(rarg), s, &f.location, StdoutCaptureType::Data)?));

          if let ASTVal::Array(vs) = rarg.variant {
            let mut i = 0;
            for v in vs {
              if is_truthy(&call_function(None, (*f).clone(), Some(v), s, fail_extern, redr, env)?) {
                res.push(AST { variant: ASTVal::Num(i as f32), location: f.location })
              }
              i += 1;
            }
          } else { unreachable!(); }

          Ok(AST { variant: ASTVal::Array(res), location: f.location })
        },
        x if x == CHARSET::Each => {
          let canned_err = error(s, fun.location.col, fun.location.len, "Rank expects integral numeric right argument.", noteformat!("The value given was:\n\r{}", v));

          match v.variant {
            ASTVal::Num(i) if i.fract() == 0.0 => ranked_fncall(*f, larg, rarg, s, i as i32, i as i32, 
env, fail_extern, redr),
            ASTVal::Array(vs) if vs.len() == 2 => {
              match vs[0].variant {
                ASTVal::Num(il) if il.fract() == 0.0 => match vs[1].variant {
                  ASTVal::Num(ir) if ir.fract() == 0.0 => ranked_fncall(*f, larg, rarg, s, il as i32, ir as i32, env, fail_extern, redr),
                  _ => Err(canned_err)
                },
                _ => Err(canned_err)
              }
            },
            _ => Err(canned_err)
          }
        },
        x if x == CHARSET::Reduce => {
          match rarg {
            Some(rarg) => {
              let l = rarg.location;
              let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &l, StdoutCaptureType::Data)?, arrayifyast(inpipe_to_ast(rarg, s, &l, StdoutCaptureType::Data)?));
              match rarg.variant {
                ASTVal::Array(mut vs) => {
                  if vs.len() == 0 {
                    Ok(nilarr())
                  } else {
                    let mut value = vs.remove(0);
                    while vs.len() > 0 {
                      let rhs = vs.remove(0);
                      value = call_function(Some(value), *f.clone(), Some(rhs), s, fail_extern, redr, env)?;
                    }
                    Ok(value)
                  }
                },
                _ => unreachable!()
              }
            },
            None => call_function(None, *f, None, s, fail_extern, redr, env)
          }
        },
        x if x == CHARSET::Scan => {
          let rarg = unoptionize(rarg);
          let l = rarg.location;
    
          let (_larg, rarg) = (inpipe_to_ast(unoptionize(larg), s, &l, StdoutCaptureType::Data)?, arrayifyast(inpipe_to_ast(rarg, s, &l, StdoutCaptureType::Data)?));

          let mut results = Vec::new();
          match rarg.variant {
            ASTVal::Array(mut vs) => {
              if vs.len() > 0 {
                results.push(vs.remove(0));
                while vs.len() > 0 {
                  let rhs = vs.remove(0);
                  results.push(
                    call_function(results.last().cloned(), *f.clone(), Some(rhs), s, fail_extern, redr, env)?
                  );
                }
              }
            },
            _ => unreachable!()
          }

          Ok(AST { variant: ASTVal::Array(results), location: l })
        },
          
        x => Err(error(s, fun.location.col, fun.location.len, format!("Unknown operator {}.", x).as_str(), "This is an internal error."))
      }
    },

    ASTVal::Apply(ls, f, rs) if match &rs { Some(s) if is_fn(&s, env) => true, _ => false } && match &ls { Some(s) if is_fn(&s, env) => true, _ => false } => {
      let lhs = call_function(larg.clone(), *ls.unwrap(), rarg.clone(), s, fail_extern, redr, env)?;
      let rhs = call_function(larg, *rs.unwrap(), rarg, s, fail_extern, redr, env)?;

      call_function(Some(lhs), *f, Some(rhs), s, fail_extern, redr, env)
    },
    
    ASTVal::Apply(ls, f, rs) if ls.is_none() && match &rs { Some(s) if is_fn(&s, env) => true, _ => false } => {
      let intermed = call_function(larg, *rs.unwrap(), rarg, s, fail_extern, redr, env)?;
      call_function(None, *f, Some(intermed), s, fail_extern, redr, env)
    },
    
    ASTVal::Apply(ls, f, rs) => {
      call_function(ls.map(|x| *x).or(larg), *f, rs.map(|x| *x).or(rarg), s, fail_extern, redr, env)
    },

    _ => Err(error(s, fun.location.col, fun.location.len, "Attempting to call non-callable.", noteformat!("The value attempting to be called was:\n\r{}", fun)))
  }
}

fn eval_command(command: AST, s: &String, env: &mut HashMap<String, AST>, fail_extern: bool) -> Result<AST, String> {
  match command.variant {
    ASTVal::Num(_) | ASTVal::String(_) | ASTVal::Symbol(_) | ASTVal::Command(..) | ASTVal::Ident(_) => Ok(command),
    ASTVal::Array(vs) => {
      let mut newvec = Vec::new();
      for v in vs {
        let newv = eval_command(v, s, env, fail_extern);
        match newv {
          Err(e) => { return Err(e); },
          Ok(o) => newvec.push(o)
        }
      }
      
      Ok(AST { variant: ASTVal::Array(newvec), location: command.location })
    }
    ASTVal::Operator(f, o, v) => {
      let val = Box::new(
        if is_fn(&v, env) {
          *v
        } else {
          eval_command(*v, s, env, fail_extern)? 
        }
      );
      Ok(AST { variant: ASTVal::Operator(f, o, val), location: command.location })
    },
    ASTVal::Apply(a, f, b) => {
      let is_lazy = is_lazy(&f);
      
      let (l, f, r) = (
        match a {
          None => None,
          Some(a) => Some(
            if is_lazy {
              *a
            } else {
              eval_command(*a, s, env, fail_extern)?
            }
          )
        },
        match f.variant {
          ASTVal::Apply(_, _, ref rs) if rs.as_ref().and_then(|rs| is_fn(&rs, env).then(|| ())).is_some() => Ok(*f),
          ASTVal::Apply(ls, ff, rs) => Ok(AST { variant: ASTVal::Apply(
            match ls.map(|ls| eval_command(*ls, s, env, fail_extern)) { Some(i) => Some(Box::new(i?)), None => None },
            Box::new(eval_command(*ff, s, env, fail_extern)?),
            match rs.map(|rs| eval_command(*rs, s, env, fail_extern)) { Some(i) => Some(Box::new(i?)), None => None }
          ), location: f.location }),
          _ => eval_command(*f, s, env, fail_extern)
        }?, 
        match b {
          None => None,
          Some(b) => Some(
            if is_lazy {
              *b
            } else {
              eval_command(*b, s, env, fail_extern)?
            }
          )
        }
      );
      
      call_function(l, f, r, s, fail_extern, Redirect { 
        stdout: ToRedirect::ToStdout,
        stderr: ToRedirect::ToStderr
      }, env)
    },

    ASTVal::Assign(n, v) => {
      let val = eval_command(*v, s, env, fail_extern)?;

      if !fail_extern { env.insert(n, val.clone()); }
      Ok(val)
    },
    ASTVal::AliasAssign(n, v) => {
      if !fail_extern { env.insert(n, *v.clone()); }
      Ok(*v)
    }

    ASTVal::SymbolList(..) => unreachable!()
  }
}

pub fn eval_commands(commands: Vec<AST>, s: &String, env: &mut HashMap<String, AST>, fail_extern: bool) -> Result<AST, String> {
  let mut v = AST { variant: ASTVal::Array(Vec::new()), location: Location { col: 0, len: 0 } };
  
  for command in commands {
    let res = eval_command(command, s, env, fail_extern);
    match res {
      Err(e) => { return Err(e); },
      Ok(r) => { 
        let l = r.location;
        v = match inpipe_to_ast(r, s, &l, StdoutCaptureType::None) {
          Err(e) => { return Err(e); },
          Ok(o) => o
        }; 
      }
    }
  }

  Ok(v)
}

pub fn resolve(node: AST, env: &HashMap<String, AST>, s: &String) -> Result<AST, String> {
  match node.variant {
    ASTVal::Ident(ref i) => {
      if token::in_operators(&i) || which(i).is_ok() { Ok(node) } else {
        match env.get(i) {
          None => Err(error(s, node.location.col, node.location.len, format!("Unknown identifier {}.", i), "This is a typo.")),
          Some(v) => Ok(v.clone())
        }
      }
    },
    
    ASTVal::Assign(name, v) => Ok(AST { variant: ASTVal::Assign(name, Box::new(resolve(*v, env, s)?)), location: node.location }),
    ASTVal::AliasAssign(name, v) => Ok(AST { variant: ASTVal::AliasAssign(name, Box::new(resolve(*v, env, s)?)), location: node.location }),

    ASTVal::Num(_) | ASTVal::Symbol(_) | ASTVal::String(_) | ASTVal::SymbolList(_) => Ok(node),

    ASTVal::Apply(a, b, c) => Ok(AST { variant: ASTVal::Apply(
      a.map(|a| resolve(*a, env, s)).map_or(Ok(None), |v| v.map(Some))?.map(Box::new),
      Box::new(resolve(*b, env, s)?),
      c.map(|c| resolve(*c, env, s)).map_or(Ok(None), |v| v.map(Some))?.map(Box::new)
    ), location: node.location }),

    ASTVal::Array(vs) => Ok(AST { variant: ASTVal::Array(
      vs.into_iter().map(|v| resolve(v, env, s)).collect::<Result<Vec<AST>, String>>()?
    ), location: node.location }),

    ASTVal::Operator(f, o, v) => Ok(AST { variant: ASTVal::Operator(Box::new(resolve(*f, env, s)?), o, Box::new(resolve(*v, env, s)?)), location: node.location }),

    ASTVal::Command(c, ags, stdin, redr) => Ok(AST { variant: ASTVal::Command(
      c,
      ags.into_iter().map(|v| resolve(v, env, s)).collect::<Result<Vec<AST>, String>>()?,
      Box::new(resolve(*stdin, env, s)?),
      redr
    ), location: node.location })
  }
}