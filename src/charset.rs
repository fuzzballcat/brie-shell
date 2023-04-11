use std::fmt;
use termion;
use unicode_width::UnicodeWidthStr;
use crate::pretty;

pub enum CHARSET {
  Assign,
  Iota,
  Plus,
  Minus,
  Times,
  Divide,
  LTack,
  RTack,
  Concat,
  Each,
  Index,
  ShapeLength,
  EnvCommand,
  EndOperator,
  Pipe,
  AntiPipe,
  Equal,
  Reduce,
  Scan,
  Greater,
  Less,
  MaxLast,
  MinFirst,
  Where,
  Iterate,
  Table,
  Selfie,
  Transpose,
  Take,
  Rotate
}


impl CHARSET {
  pub fn as_str(&self) -> &'static str {
    match self {
      // FORM
      CHARSET::EnvCommand => ")",
      CHARSET::EndOperator => ";",
      CHARSET::Assign => ";",
      CHARSET::Pipe => "|",
      CHARSET::AntiPipe => "]",

      // OPERATOR
      CHARSET::Each => "$",
      CHARSET::Reduce => "/",
      CHARSET::Scan => "\\",
      CHARSET::Where => "?",
      CHARSET::Iterate => "~",
      CHARSET::Table => "`",
      CHARSET::Selfie => "'",

      // FUNCTION
      CHARSET::Iota => "!",
      CHARSET::Plus => "+",
      CHARSET::Minus => "-",
      CHARSET::Times => "*",
      CHARSET::Divide => "%",
      CHARSET::LTack => "{",
      CHARSET::RTack => "}",
      CHARSET::Concat => ",",
      CHARSET::Index => "[",
      CHARSET::ShapeLength => "#",
      CHARSET::Equal => "=",
      CHARSET::Greater => "<",
      CHARSET::Less => ">",
      CHARSET::MaxLast => "^",
      CHARSET::MinFirst => "&",
      CHARSET::Transpose => "@",
      CHARSET::Take => ":",
      CHARSET::Rotate => "."
    }
  }

  // refcard
  pub fn describe(&self) -> &'static str {
    match self {
      CHARSET::EnvCommand => "Run an environment command.",
      CHARSET::EndOperator => "Assignment.", // a little cheating
      CHARSET::Assign => "Alias assignment.",
      CHARSET::Pipe => "Pipe (group left).",
      CHARSET::AntiPipe => "Antipipe (group right).",

      CHARSET::Each => "Apply f at depth i or respective depths A.",
      CHARSET::Reduce => "Fold the argument from the left with f.",
      CHARSET::Scan => "Accumulate applying f from the left.",
      CHARSET::Where => "Find where true.",
      CHARSET::Iterate => "Find fixpoint of function.\nIterate function until condition is met.",
      CHARSET::Table => "Outer product (apply between combinations).",
      CHARSET::Selfie => "Repeat argument.",

      CHARSET::Iota => "Produce a sequence.\nSplit-partition on zeroes.",
      CHARSET::Plus => "Addition.",
      CHARSET::Minus => "Subtraction.\nString to array.",
      CHARSET::Times => "Multiplication.\nLogical not.",
      CHARSET::Divide => "Division.",
      CHARSET::Equal => "Check equality.",

      CHARSET::LTack => "Choose left.",
      CHARSET::RTack => "Choose right.",
      CHARSET::Concat => "Concatenate elements.",
      CHARSET::Index => "Choose at index.",
      CHARSET::ShapeLength => "Reshape array.\nCount length.",
      CHARSET::Greater => "Comparison.\nSort ascending.",
      CHARSET::Less => "Comparsion.\nSort descending.",
      CHARSET::MaxLast => "Last element.\nMaximum (OR).",
      CHARSET::MinFirst => "First element.\nMinimum (AND).",
      CHARSET::Transpose => "Transpose axes.\nReverse all axes.",
      CHARSET::Take => "First element of a list.\nTake n elements from a list.",
      CHARSET::Rotate => "Reverse array.\nRotate array."
    }
  }

  pub fn definitions() -> &'static str {
    "A,B,C — array  |  f,g,h — function  |  i,j,k — scalar\nE — an evaluated expression  |  U — an unevaluated expression"
  }
  
  pub fn example(&self) -> &'static str {
    match self {
      CHARSET::EnvCommand => ")command [arguments]",
      CHARSET::EndOperator => "name; E",
      CHARSET::Assign => "name;; U",
      CHARSET::Pipe => "E | f",
      CHARSET::AntiPipe => "f ] E",

      CHARSET::Each => "f$ iA ;",
      CHARSET::Reduce => "f/ A",
      CHARSET::Scan => "f\\ A",
      CHARSET::Where => "f? A",
      CHARSET::Iterate => "f~\nf~iAg;",
      CHARSET::Table => "A f` B",
      CHARSET::Selfie => "f' A",

      CHARSET::Iota => "! i\nA ! B",
      CHARSET::Plus => "i + j",
      CHARSET::Minus => "i - j\n- i",
      CHARSET::Times => "i * j\n* i",
      CHARSET::Divide => "i % j",
      CHARSET::Equal => "i = j",
      CHARSET::ShapeLength => "# A\nA # B",

      CHARSET::LTack => "A { B",
      CHARSET::RTack => "A } B",
      CHARSET::Concat => "A , B",
      CHARSET::Index => "A [ i",
      CHARSET::Greater => "i < j\n< A",
      CHARSET::Less => "i > j\n> A",
      CHARSET::MaxLast => "^ A\ni ^ j",
      CHARSET::MinFirst => "& A\ni & j",
      CHARSET::Transpose => "A @ B\n@ A",
      CHARSET::Take => ": A\ni : A",
      CHARSET::Rotate => ". A\ni . A"
    }
  }

  pub fn iterator() -> impl Iterator<Item=&'static CHARSET> + 'static {
    static CHARS: [CHARSET; 30] = [
      // FORM
      CHARSET::EnvCommand,
      CHARSET::EndOperator,
      CHARSET::Assign,
      CHARSET::Pipe,
      CHARSET::AntiPipe,

      // OPERATOR
      CHARSET::Each,
      CHARSET::Reduce,
      CHARSET::Scan,
      CHARSET::Where,
      CHARSET::Iterate,
      CHARSET::Table,
      CHARSET::Selfie,

      // FUNCTION
      CHARSET::Iota,
      CHARSET::Plus,
      CHARSET::Minus,
      CHARSET::Times,
      CHARSET::Divide,
      CHARSET::Equal,
      CHARSET::LTack,
      CHARSET::RTack,
      CHARSET::Concat,
      CHARSET::Index,
      CHARSET::ShapeLength,
      CHARSET::Greater,
      CHARSET::Less,
      CHARSET::MaxLast,
      CHARSET::MinFirst,
      CHARSET::Transpose,
      CHARSET::Take,
      CHARSET::Rotate
    ];
    CHARS.iter()
  }
}

pub fn refcard() -> String {
  let mut out = String::new();
  out += "\x1b[0m";
  out += CHARSET::definitions();
  out += "\x1b[32m\n\n";

  let max = CHARSET::iterator().map(|f| f.example().split("\n").map(|l| l.width()).max().unwrap_or(0)).max().unwrap_or(0);

  let mut alternate = false;
  for c in CHARSET::iterator() {
    if alternate {
      out += "\x1b[1m";
    }
    
    let e = c.example();
    let d = c.describe();
    out += pretty::stitch(vec![e.to_string(), d.to_string()], " ".repeat(max - e.split("\n").map(|l| l.width()).max().unwrap_or(0) + 2).as_str()).as_str();

    if alternate {
      out += "\x1b[0;32m";
    }
    
    out += "\n\r";
    
    alternate = !alternate;
  }
  out.pop(); out.pop();
  
  out
}

pub fn detailedinfo<S: AsRef<str>>(i: S) -> String {
  match i.as_ref() {
    "builtins" => format!("\n  pipe      Perform pipe redirection.\n  collect   Collect stdout, stderr, and exit code.\n  num       Convert between string and numeric representations.\n  exit      Exit.\n  list      Convert between bash-style list format and data structures.\n  csv       Convert between CSV and data structures.\n  json      Convert between JSON and data structures."),
    "list" => format!("Convert between bash-style lists and data structures.\n\nImport:\nWith a string argument, list parses the string into a corresponding data structure.\n\n     list \"1\\t2\\n3\"\n  => ┌2──────┐\n     │┌1────┐│\n     ││╭\"──╮││\n     │││ 1 │││\n     ││╰───╯││\n     ││╭\"──╮││\n     │││ 2 │││\n     ││╰───╯││\n     │└─────┘│\n     │╭\"──╮  │\n     ││ 3 │  │\n     │╰───╯  │\n     └───────┘\n\nExport:\nWith a list argument, list converts the array (max depth 2) to a corresponding serialized form.\n\n     list (1 2) 3\n  => ╭\"─────╮\n     │ 1  2 │\n     │ 3    │\n     ╰──────╯"),
    "csv" => format!("Convert between CSV format and data structures.\n\nImport:\nWith a string argument, CSV parses the string into a corresponding data structure.\n\n     csv \"1\\n2,3\"\n  => ┌2──────┐\n     │┌1────┐│\n     ││╭\"──╮││\n     │││ 1 │││\n     ││╰───╯││\n     │└─────┘│\n     │┌1────┐│\n     ││╭\"──╮││\n     │││ 2 │││\n     ││╰───╯││\n     ││╭\"──╮││\n     │││ 3 │││\n     ││╰───╯││\n     │└─────┘│\n     └───────┘\n\nExport:\nWith a list argument, csv converts the array (max depth 2) to a corresponding serialized form.\n\n     csv (1 2) (3 4) 5\n  => ╭\"────╮\n     │ 1,2 │\n     │ 3,4 │\n     │ 5   │\n     │     │\n     ╰─────╯\n\nA custom separator can be specified as the left argument in either mode."),
    "json" => format!("Convert between JSON format and data structures.\n\nImport:\nWith a string argument, JSON parses the string into a corresponding data structure.  Objects are converted to an array of keys before an array of values.\n\n     json \"{{\\\"foo\\\":2, \\\"bar\\\":4}}\"\n  => ┌2────────┐\n     │┌1──────┐│\n     ││╭\"────╮││\n     │││ foo │││\n     ││╰─────╯││\n     ││╭\"────╮││\n     │││ bar │││\n     ││╰─────╯││\n     │└───────┘│\n     │┌1──┐    │\n     ││2 4│    │\n     │└───┘    │\n     └─────────┘\n\nExport:\nWith a list argument, json converts the array to a corresponding serialized form.\n\n     json 1 2 3\n  => ╭\"────────╮\n     │ [1,2,3] │\n     ╰─────────╯"),
    "exit" => format!("Exit.  Optionally takes an integral right argument to specify the exit code."),
    "pipe" => format!("Redirect a process' output.  Has no effect on non-shell functions.\n\npipe takes a process as its left argument and a list of symbols on its right.\nSymbols are processed by paired in twos, with each first symbol being the \"from\" of the redirect and each second symbol being the \"to\" of the redirect.  Available symbols are:\n\n  -o    STDOUT\n  -e    STDERR\n  -n    NULL (/dev/null)\n\nIn addition, there are two macro symbols.\n\n  --swap    Equivalent to -oe -eo.\n  --null    Equivalent to -on -en.\n\nExample usage:\n     cat \"missing.file\" | pipe --swap | sed -u \"s/.*/{{&}}/\" | pipe --swap\n  => {{cat: missing.file: No such file or directory}}"),
    "collect" => format!("Collect a process' stdout, exit code, and stderr (in this order) in an array.  Returns an unchanged value for non-shell functions.\n\n     cat \"missing.file\" | collect\n  => ┌1──────────────────────────────────────────────────────┐\n     │╭\"─╮ 1 ╭\"─────────────────────────────────────────────╮│\n     ││  │   │ cat: missing.file: No such file or directory ││\n     │╰──╯   │                                              ││\n     │       ╰──────────────────────────────────────────────╯│\n     └───────────────────────────────────────────────────────┘"),
    "num" => format!("Convert between string and number.\n\n    num \"5\"\n  => 5\n\n     num 5\n  => ╭\"──╮\n     │ 5 │\n     ╰───╯"),
    
    "(" => format!("Open parenthesis.  (Really?)"),
    ")" => format!("Close parenthesis.  When found at the start of a line, indicates a shell command.  Available shell commands are:\n\n  \x1b[0;1m)help [command?]\x1b[0;32m\n  \x1b[0;1m)info [expression]\x1b[0;32m\n  \x1b[0;1m)wipe\x1b[0;32m\n  \x1b[0;1m)clear\x1b[0;32m\n  \x1b[0;1m)rtf [filename]\x1b[0;32m"),
    ")help" => format!("Get help for a command.  (You're doing it.)"),
    "repl" => format!("Available repl commands are:\n\n 
 )info\n  )wipe\n  )clear\n  )rtf\n  )help\n  )cm\n  )c"),
    ")cm" => format!("Swap commit mode between manual and automatic.  Automatic commit mode adds every valid command to the `)rtf` history buffer.  Manual commit mode only adds commands prefixed with `)c` or commands where the next line is a `)c` invocation."),
    ")c" => format!("Commit to the `)rtf` buffer.  Either commit this line if it is not empty past `)c`, or commit the previous line."),
    ")info" => format!(")info [expression] displays a tree version of a program line.  This indicates only the parsed form of the line, not the value resulting from executing the line.\n\nTrees are constructed according to the following rules:\n\n  - A fork in the tree represents function application.\n\n    f\n    ├──┐\n    l  r\n\n    The function may be enclosed in square dotted brackets if it is complex.\n\n  - Items side-by-side indicate an array of items.\n    Items under a \x1b[0;1m[]\x1b[0;32m symbol also indicate an array (in this case, nested)."),
    ")wipe" => format!("Wipe the current recorded buffer used by )rtf.  Previous line history is still accessible, but (unless run again) will not appear in )rtf output."),
    ")clear" => format!("Clear the screen.  This does not destroy any history."),
    ")rtf" => format!("Repl to file.  )rtf [filename] opens an editor which allows the user to choose which lines in the current )rtf history buffer (by default, the repl's line history) to write to the file specified in the command invokation.\n\nKeymap for editor:\n\n  - UP/DOWN ARROW: Scroll through lines.\n  - BACKSPACE: Toggle line inclusion.\n  - ESC: Exit editor and cancel write."),
    x if x == CHARSET::Plus => format!("Arithmetic addition.\n\n     6 {0} 7\n  => 13\n\nString concatenation.\n\n     \"Hello\" {0} \", world\"\n  => ╭\"─────────────╮\n     │ Hello, world │\n     ╰──────────────╯", CHARSET::Plus),
    x if x == CHARSET::Minus => format!("Arithmetic subtraction.\n\n     6 {} 7\n  => -1\n\nConvert a string to characters.\n\n     -\"foo\"\n  => ┌1────┐\n     │╭\"──╮│\n     ││ f ││\n     │╰───╯│\n     │╭\"──╮│\n     ││ o ││\n     │╰───╯│\n     │╭\"──╮│\n     ││ o ││\n     │╰───╯│\n     └─────┘", CHARSET::Minus),
    x if x == CHARSET::Times => format!("Arithmetic multiplication.\n\n     6 {0} 7\n  => 42\n\nLogical negation.\n\n     {0} 1\n  => 0", CHARSET::Times),
    x if x == CHARSET::Divide => format!("Arithmetic division.\n\n     6 {} 7\n  => 0.85714287", CHARSET::Divide),
    x if x == CHARSET::Equal => format!("Equality.\n\n     6 {} 1 6 3 2 6\n  => ┌1────────┐\n     │0 1 0 0 1│\n     └─────────┘", CHARSET::Equal),
    
    x if x == CHARSET::LTack => format!("Choose the left argument.  Left tack ignores its right argument.\n\n     3 2 {0} 5 4\n  => ┌1──┐\n     │3 2│\n     └───┘\n\nGet the first element of an array.\n\n     {0} 1 2 3 4\n  => 1", CHARSET::LTack),
    x if x == CHARSET::RTack => format!("Choose the right argument.  Right tack ignores its left argument.\n\n     3 2 {0} 5 4\n  => ┌1──┐\n     │5 4│\n     └───┘\n\nGet the last element of an array.\n\n     {0} 1 2 3 4\n  => 4", CHARSET::RTack),
    x if x == CHARSET::Concat => format!("Join two arrays.\n\n     (1 2) {0} (3 4)\n  => ┌1──────┐\n     │1 2 3 4│\n     └───────┘\n\nEnlist a value.\n\n     {0} 4\n  => ┌1┐\n     │4│\n     └─┘", CHARSET::Concat),
    x if x == CHARSET::Index => format!("Choose from array at index.\n\n     3 {} 1 2 3 4\n  => 4", CHARSET::Index),
    x if x == CHARSET::ShapeLength => format!("Count the lengths of an array at each depth, taking the maximum for each depth.\n\n     {0} ((1 2 ({1}3))(4 5 6))\n  => ┌1────┐\n     │2 3 1│\n     └─────┘\n\nReshape one array to have axes of the given lengths, wrapping as needed.\n\n     2 3 {} 1 2 3 4\n  => ┌2──────────────┐\n     │┌1────┐ ┌1────┐│\n     ││1 2 3│ │4 1 2││\n     │└─────┘ └─────┘│\n     └───────────────┘", CHARSET::ShapeLength, CHARSET::Concat),

    x if x == CHARSET::Each => format!("Apply a function at specified depth to its arguments.\n\n     (1 3) {0}{1}1{2} (2 4)\n  => ┌2──────────┐\n     │┌1──┐ ┌1──┐│\n     ││1 2│ │3 4││\n     │└───┘ └───┘│\n     └───────────┘\n\nApply a function at a specified depth to its arguments, respectively.\n\n     (1 3) {0}{1}1 0{2} (2 4)\n  => ┌2──────────────┐\n     │┌1────┐ ┌1────┐│\n     ││1 2 4│ │3 2 4││\n     │└─────┘ └─────┘│\n     └───────────────┘", CHARSET::Concat, CHARSET::Each, CHARSET::EndOperator),
    x if x == CHARSET::Reduce => format!("Apply a function between each element of an array, grouping from the left.\n\n     {}{}{} 1 2 3 4\n  => 10", CHARSET::Plus, CHARSET::Reduce, CHARSET::EndOperator),

    x if x == CHARSET::Assign => format!("Evaluate an expression and assign it to a valid name.\n\n     a{0}2 {1} 4\n     a {1} 3\n  => 9\n\nAssign an expression to a name without evaluating it.\n\n     b{0}{0}echo -n \"Eval\"\n     (b) {1} \"uated\"\n  => ╭\"──────────╮\n     │ Evaluated │\n     ╰───────────╯\n\nNote the parenthetical enclosure of the variable.  This is critical to force application within the value, as the variable might otherwise be treated as a partially-applied function.", CHARSET::Assign, CHARSET::Plus),

    x if x == CHARSET::Pipe => format!("Fix precedence of a left-side value.\n\n     2 {1} 3 {0} {1} 4\n  => 9", CHARSET::Pipe, CHARSET::Plus),
    x if x == CHARSET::AntiPipe => format!("Fix precedence of a right-side value.\n\n     4 {1} {0} 3 {1} 2 \n  => 9", CHARSET::AntiPipe, CHARSET::Plus),

    x if x == CHARSET::Iota => format!("Produce a sequence of consecutive integers.\n\n     {0} 4\n  => ┌1──────┐\n     │0 1 2 3│\n     └───────┘\n\nSplit on zero.\n\n     1 1 0 1 0 1 {0} 1 2 3 4 5 6\n  => ┌2────────────┐\n     │┌1──┐ ┌1┐ ┌1┐│\n     ││1 2│ │4│ │6││\n     │└───┘ └─┘ └─┘│\n     └─────────────┘", CHARSET::Iota),
    x if x == CHARSET::Where => format!("Find indices of occurances where a predicate returns true.\n     ({1}3){0} 1 4 3 1 3 3 5\n  => ┌1────┐\n     │2 4 5│\n     └─────┘", CHARSET::Where, CHARSET::Equal),

    x if x == CHARSET::Greater => format!("Comparison.\n\n     5 {0} 2\n  => 0\n\nFind indices that would sort an array in ascending order.\n\n     {0} 1 4 3 0\n  => ┌1──────┐\n     │3 0 2 1│\n     └───────┘", CHARSET::Greater),
    x if x == CHARSET::Less => format!("Comparison.\n\n     5 {0} 2\n  => 1\n\nFind indices that would sort an array in descending order.\n\n     {} 1 4 3 0\n  => ┌1──────┐\n     │1 2 0 3│\n     └───────┘", CHARSET::Less),

    x if x == CHARSET::MaxLast => format!("Maximum (OR).\n\n     5 {0} 2\n  => 5\n\nChoose last element.\n\n     {0} 1 4 3 2\n  => 2", CHARSET::MaxLast),
    x if x == CHARSET::MinFirst => format!("Minimum (AND).\n\n     5 {0} 2\n  => 2\n\nChoose last element.\n\n     {0} 1 4 3 2\n  => 1", CHARSET::MinFirst),

    x if x == CHARSET::Iterate => format!("Apply a function n times.  The left argument will be applied as a constant, if provided.\n\n     1 {1}{0}4{2} 3\n  => 7\n\nThe value to iterate can be an array, resulting in an array of appropriately-applied results.\n\nNote that iterate also accepts a function as its value, which will be first applied to the left and right arguments to determine the iteration count.\n\n     1 {1}{0}{3}{2} 3\n  => 6\n\nFind the fixed point of a function.  (Apply a function until its value equals the previous result.)\n\n     ({4}100){0} 2\n  => 0", CHARSET::Iterate, CHARSET::Plus, CHARSET::EndOperator, CHARSET::RTack, CHARSET::Divide),

    x if x == CHARSET::Selfie => format!("Return a derived function which accepts a single argument and uses it as both its left and right values.\n\n     {1}{0} 4\n  => 16", CHARSET::Selfie, CHARSET::Times),

    x if x == CHARSET::Table => format!("Apply a function between each element of the left side and each element of the right side (i.e, all combinations of values from the left and right arguments).\n\n     {1}{0}{2} {3}2\n  => ┌2────┐\n     │┌1──┐│\n     ││0 1││\n     │└───┘│\n     │┌1──┐│\n     ││1 2││\n     │└───┘│\n     └─────┘", CHARSET::Table, CHARSET::Plus, CHARSET::Selfie, CHARSET::Iota),

    x if x == CHARSET::Transpose => format!("Tranpose axes of an array.  Left argument indicates the order of axes.\n\n     1 0@(1 2) (3 4)\n  => ┌2────┐\n     │┌1──┐│\n     ││1 3││\n     │└───┘│\n     │┌1──┐│\n     ││2 4││\n     │└───┘│\n     └─────┘\n\nOmitting the left argument results in a reversal of axes.\n\n     @(1 2) (3 4)\n  => ┌2────┐\n     │┌1──┐│\n     ││1 3││\n     │└───┘│\n     │┌1──┐│\n     ││2 4││\n     │└───┘│\n     └─────┘"),

    x if x == CHARSET::Take => format!("Choose the first element of an array.\n\n     :2 3 1\n  => ┌1┐\n     │2│\n     └─┘\n\nTake n elements from the start of an array.  If the count is negative, take from the end.\n\n     -2:4 2 3 5\n  => ┌1──┐\n     │3 5│\n     └───┘"),

    x if x == CHARSET::Rotate => format!("Reverse an array.\n\n     . 1 2 3\n  => ┌1────┐\n     │3 2 1│\n     └─────┘\n\nRotate an array.  Positive means leftward movement, negative means rightward movement.\n\n     2 . 1 2 3 4 5\n  => ┌1────────┐\n     │3 4 5 1 2│\n     └─────────┘"),
    

    x if x == "language" => format!("\x1b[0;1;4mBrie Shell Language Tutorial\x1b[0;32m\n\nThe Brie Shell is an interactive language.  Each line is an expression which evaluates to a result.  The result of an expression's evaluation in this tutorial will be shown on the following line after =>.  In the REPL, it is simply shown on the following line.\n\nStandard arithmetic operators apply.  Note however that division is represented by %.\n\n     1 + 2\n  => 3\n\n     6 % 3\n  => 2\n\n     \"Hello, \" + \"world!\"\n  => ╭\"──────────────╮\n     │ Hello, world! │\n     ╰───────────────╯\n\nBrie Shell is an array language.  Arrays are written without notation simply by juxtaposition.  The REPL displays arrays within boxes.  The number at the top indicates the \"depth\" of the array, i.e. how many arrays can be found nested inside it.\n\n     1 2 3 4\n  => ┌1──────┐\n     │1 2 3 4│\n     └───────┘\n\nBrie Shell is an array language.  Functions are \"depth-polymorphic\" in that they may be applied to arrays all at once.  This applies even if the arrays are nested, which can be written using parentheses.\n\n     (1 2) (3 4) + (5 6) (7 8)\n  => ┌2──────┐\n     │┌1──┐  │\n     ││6 8│  │\n     │└───┘  │\n     │┌1────┐│\n     ││10 12││\n     │└─────┘│\n     └───────┘\n\nIt's possible to apply a function \"between\" elements of an array using the Reduce modifier, `/`.  This is called Reduce because it collapses an array using the function.\n\n     +/ 1 2 3 4\n  => 10\n\nNotice that the modifier is placed after the function, `+`.\nThere is another modifier which performs a similar function, Scan.\n\n     +\\ 1 2 3\n  => ┌1────┐\n     │1 3 6│\n     └─────┘\n\nNote that Scan returns an array as if reduce had been applied to just the first element of array, then the first and second element, then the first and second and third, and so on.\n\nIn addition to functions on single objects, we can manipulate entire arrays.  Concat, `,`, joins two arrays together.  Grade Up and Grade Down, `<` and `<`, respectively, returns a list of indices that, if the elements put in that order, would sort the array.  Such functions will not be described in detail here, as they can be learned through careful use of the refcard and the )help command.\n\nIn some situations, one may wish to apply an array-oriented function to each inner array rather than an array as a whole, or a scalar-oriented function to an entire array, and so on.  This can be achieved through usage of the modifier Depth, `$`.  Depth takes an argument which specifies the depth \"downward\" to traverse in the array, starting at zero.  A negative number specifies how \"high\" up from the bottom to go to traverse the array, starting at zero.\n\n     < (3 2) (1 7)\n  => ┌1──┐\n     │0 1│\n     └───┘\n     || by the way, this is a comment\n     || not intended behavior — sorts the entire array\n\n     <$1; (3 2) (1 7)\n  => ┌2────┐\n     │┌1──┐│\n     ││1 0││\n     │└───┘│\n     │┌1──┐│\n     ││0 1││\n     │└───┘│\n     └─────┘\n\nHere we see the syntax for the usage of a modifier which takes a value — the value is placed after the modifier and followed by a semicolon.  Any value, including an array, is allowed here.  Additionally, a function may be used, which is evaluated with the left and right arguments of the whole expression to yield a result.  For example, the following invocation evaluates at depth 1 because the result of evaluating Shape `#` on the right argument (there is no left argument) is 1.\n\n     -$#; (,1 2)\n  => ┌2──────┐\n     │┌1────┐│\n     ││-1 -2││\n     │└─────┘│\n     └───────┘\n\n(Note that this of course is irrelevant, as `-` already applies at scalar [maximum] depth.)\n\nIt is useful to make more complicated functions out of existing functions, as in building blocks.  This is done through the formation of tacit trains.  Tacit trains follow two rules:\n  1. Multiple functions in a row are applied in succession.\n    5 -+ 2 is equivalent to -(5 + 2)\n  2. If there are 3 or more functions in a train, the outer two form a \"fork\".  Each of the outer two is applied to the arguments, then the middle is applied between the two results.\n    5 -+* 2 is equivalent to (5 - 2) + (5 * 2)\n\nTrains are critical to forming any useful experession.  For example, we can write the greater-than-or-equal-to operator as >^= simply with the definitions of Greater `>`, Or `^`, and Equals `=`.  Or we could even write not-greater-than-or-equal-to as >*^= (noting * to be unary not) — but this is of course simply the less-than operator.\n\nThe final aspect of creating functional forms is partial application.  Suppose we wish to find the indices of an array where the values are greater than five.  We can use the Where `?` modifier to do this, but that requires creating a function that returns true for values greater than five.  Doing so involves \"binding\" the function `>` to the right-side value `5`.\n\nIn the Brie shell, binding a function to a value in this way (partial application) looks little different than applying a function to a value.  In fact, the following example works just as expected:\n\n     (> 5)? 1 2 6 3 10\n\n  => ┌1──┐\n     │2 4│\n     └───┘\n\nIt is however generally bad practice to write functions in this way because it is not guaranteed that they are to work.  There are cases in which it is impossible to disambiguate whether the call is intended to produce a value or a partially-applied function, such as in the expression `(> 5) }} 1 2 3`.  Brie uses semantic whitespace to disambiguate such instances — functions written without surrounding whitespace will be treated as partial application, while functions with whitespace will be treated as standard function calls.  Thus, the above example should be written `(>5)? 1 2 6 3 10` so as to cause no confusion (and the previous example written `(>5)}} 1 2 3`).\n\nIt is often useful to give names to expressions or patterns that are used again.  This can be done through the assignment formation, `;`.\n\n     a_name;15\n     a_name + 27\n  => 42\n\n     Note that `;` evaluates the expression before binding it to a name, making it impossible to assign a name to a functional form using `;`.  For that, the lazy binding form may be used, `;;`.  `;;` does not evaluate the expression before assigning it to a name.\n\n     a_fn;; -+\n     1 a_fn 2\n  => -3\n\n\x1b[4mUsing Brie as a Shell\x1b[0;32m\n\nAs a true shell, any identifier found in $PATH will be executed as a shell command.  To faciliate command usage, members of the symbol datatype may be constructed as in bash.  `-` may be followed by any number of single characters to form a list of symbols, while `--` may be followed by characters to form a single multi-character symbol.  Note that in a symbol list, the symbols will be assimilated into the enclosing array, facilitating constructions such as `-ab -c` evaluating to `-a -b -c`.\n\n     ls -aS --color\n  => [output of ls with all files, sorted by size, and in color]\n\nBrie defines a pipe operator as in bash.  However, the pipe operator is not a special form; it is merely syntactical sugar for grouping application on the left.  \n\n     echo \"foo\" | cat\n  => foo\n\nNote that commands receive their STDIN as a left argument and their ARGS as a right argument.\n\nJust as the pipe operator groups leftward, the antipipe operator groups rightward.\n\n     cat [ echo \"some.file\"\n  => [contents of some.file]\n\nA final note on pipe: as pipe and antipipe are not special forms, they work equally well on non-shell functions as they do on shell functions.\n\nBrie defines the `collect` and `redirect` primitives for manipulating output from shell commands.  Info for these can be found in the )help docs.\n\n\x1b[4mUsing the REPL\x1b[0;32m\n\nThe Brie repl itself has certain commands which can be used to affect the operation of the REPL.  Details for each can be found by invoking `)help repl`.\n\nShell commands begin with `)` and are followed by a word.  `)help` itself is a shell command.\n\nThe Brie shell keeps a running history of valid, executed commands.  This history can be written to a file by using `)rtf`, allowing one to construct a shell script simply by interacting with the REPL in real time.  `)rtf` also provides an editor to remove unwanted lines.\nWhile `)rtf` by default includes every executed line in its history, this can be changed to remove unnecessary clutter.  By invoking `)cm`, the \"commit mode\" is switched between automatic and manual.  Automatic (default) mode commits every valid line, while manual mode requires a line to be prefixed with `)c` or followed by a single line of `)c` to be added to the history session.\n\n)wipe can be used to empty the history buffer.\n\n_____ . . . _____"),
    
    _ => format!("\x1b[1;31mUnknown item.\x1b[0;32m")
  }
}

impl From<&CHARSET> for &str {
  fn from(charset: &CHARSET) -> &'static str {
    charset.as_str()
  }
}

impl PartialEq<&str> for CHARSET {
  fn eq(&self, other: &&str) -> bool {
    <&CHARSET as Into<&str>>::into(self) == *other
  }
}

impl PartialEq<CHARSET> for &str {
  fn eq(&self, other: &CHARSET) -> bool {
    *self == <&CHARSET as Into<&str>>::into(other)
  }
}

impl PartialEq<String> for CHARSET {
  fn eq(&self, other: &String) -> bool {
    <&CHARSET as Into<&str>>::into(self) == *other
  }
}

impl PartialEq<CHARSET> for String {
  fn eq(&self, other: &CHARSET) -> bool {
    *self == <&CHARSET as Into<&str>>::into(other)
  }
}

impl fmt::Display for CHARSET {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

pub enum Colors {
  Number,
  Operator,
  BuiltinName,
  String,
  Symbol,
  Reset,
  Comment
}

impl From<Colors> for String {
  fn from(color: Colors) -> String {
    format!("{}", color)
  }
}

impl fmt::Display for Colors {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Colors::Number => write!(f, "{}", termion::color::Fg(termion::color::Blue)),
      Colors::Operator => write!(f, "{}", termion::color::Fg(termion::color::Red)),
      Colors::BuiltinName => write!(f, "{}", termion::color::Fg(termion::color::Yellow)),
      Colors::String => write!(f, "{}", termion::color::Fg(termion::color::Green)),
      Colors::Symbol => write!(f, "{}", termion::color::Fg(termion::color::Magenta)),
      Colors::Comment => write!(f, "{}", termion::color::Fg(termion::color::Green)),
      Colors::Reset => write!(f, "{}", termion::color::Fg(termion::color::White))
    }
  }
}
