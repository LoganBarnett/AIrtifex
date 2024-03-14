{
  description = "";
  inputs = {
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = github:NixOS/nixpkgs/nixpkgs-unstable;
  };

  outputs = { self, fenix, nixpkgs }@inputs: {

    devShells.aarch64-darwin.default = let
      system = "aarch64-darwin";
      rust-toolchain = fenix.packages.${system}.latest.toolchain;
      pkgs = import nixpkgs {
        inherit system;
        overlays = [
          (final: prev: {
            rustPlatform = (prev.makeRustPlatform {
              cargo = rust-toolchain;
              rustc = rust-toolchain;
            });
          })
        ];
      };
    in pkgs.mkShell {
      buildInputs = [
        pkgs.cargo
        pkgs.rustc
        pkgs.libiconv
      ];
    };

  };
}
