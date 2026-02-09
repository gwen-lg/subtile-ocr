{ pkgs ? import <nixpkgs> { } }:

let
  inherit (pkgs) lib;
  rustPlatform = pkgs.rustPlatform;
  cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "subtile-ocr";
  version = cargoToml.package.version;

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
    llvmPackages.clang
    llvmPackages.libclang
  ];

  buildInputs = with pkgs; [
    leptonica
    tesseract5
  ];

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  meta = with lib; {
    description = "Converts DVD VOB subtitles to SRT subtitles with Tesseract OCR";
    homepage = "https://github.com/gwen-lg/subtile-ocr";
    license = licenses.gpl3Only;
    mainProgram = "subtile-ocr";
    platforms = platforms.unix;
  };
}
