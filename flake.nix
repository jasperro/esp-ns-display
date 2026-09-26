{
  description = "ESP32 (Xtensa) NS Departure Board Dev Environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    esp-rs-nix = {
      url = "github:leighleighleigh/esp-rs-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      esp-rs-nix,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        esp-rs = esp-rs-nix.packages.${system}.esp-rs;
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            stdenv.cc
            pkg-config
            rustup
            rust-analyzer
            espflash
            ldproxy
            libiconv
          ];

          shellHook = ''
            export PS1="(esp32-devshell) $PS1"
            # Point rustup and rust-analyzer directly to the Nix esp-rs toolchain
            export RUSTUP_TOOLCHAIN="${esp-rs}"
            export RUST_SRC_PATH="${esp-rs}/lib/rustlib/src/rust/library"

            echo "ESP32 Xtensa dev shell loaded!"
            echo "Toolchain: ${esp-rs}"
          '';
        };
      }
    );
}
