{
  description = "mithril";
  inputs = {
    nixpkgs-stable.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs-stable";
      };
    };
    pre-commit-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs-stable.follows = "nixpkgs-stable";
    };
  };
  outputs =
    {
      self,
      rust-overlay,
      nixpkgs-stable,
      flake-utils,
      pre-commit-hooks,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs-stable.legacyPackages.${system}.extend (import rust-overlay);
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        buildInputs = [
          rustToolchain
          self.checks.${system}.pre-commit-check.enabledPackages
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit (self.checks.${system}.pre-commit-check) shellHook;
          inherit buildInputs;
        };

        formatter = pkgs.nixfmt-rfc-style;
        checks = {
          pre-commit-check = pre-commit-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              clippy.enable = true;
              rustfmt.enable = true;
              cargo-check.enable = true;
            };
          };
        };

      }
    );
}
