# brie-shell
The Brie shell.  A logical, array-based shell with terse but familiar syntax.

## Installation Instructions
For most Linux or Mac systems, the `brie-linux` or `brie-mac` executable should suffice.
If not, Brie can be built from source by:
- Cloning the repo
- Installing suitable Rust build tools, i.e. `cargo`, `rustup`
- Running `cargo build --release` in the repo directory

The executable is both the shell (when supplied without arguments) and a file executor (when supplied arguments), as is bash.  You may follow standard directions for your system to change the default shell using the Brie executable.

## Usage
Being a shell, Brie is self-descriptive.  Try `)help` to start.
