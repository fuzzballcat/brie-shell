{ pkgs }: {
	deps = [
  pkgs.python3Minimal
  pkgs.rustc
		pkgs.rustfmt
		pkgs.cargo
		pkgs.cargo-edit
        pkgs.rust-analyzer
	];
}