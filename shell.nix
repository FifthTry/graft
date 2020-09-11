let
  system = import <nixpkgs> {};
  moz_overlay = import (
    builtins.fetchTarball {
      url = https://s3.ap-south-1.amazonaws.com/downloads.fifthtry.com/nixpkgs-mozilla-efda5b357451dbb0431f983cca679ae3cd9b9829.tar.gz;
      sha256 = "11wqrg86g3qva67vnk81ynvqyfj0zxk83cbrf0p9hsvxiwxs8469";
    }
  );
  nixpkgs = import (
    builtins.fetchTarball {
      url = https://s3.ap-south-1.amazonaws.com/downloads.fifthtry.com/nixpkgs-20.03.tar.gz;
      sha256 = "0yn3yvzy69nlx72rz2hi05jpjlsf9pjzdbdy4rgfpa0r0b494sfb";
    }
  ) {
    overlays = [ moz_overlay ];
    config = { allowUnfree = true; };
  };
  frameworks = nixpkgs.darwin.apple_sdk.frameworks;
  rust = (
    nixpkgs.rustChannelOf {
      rustToolchain = ./rust-toolchain;
    }
  ).rust.override {
    extensions = [
      "clippy-preview"
      "rust-src"
    ];
  };
in
  with nixpkgs;

  stdenv.mkDerivation {
    name = "graft-env";
    buildInputs = [ rust ];

    nativeBuildInputs = [
      file
      zsh
      wget
      which
      locale
      vim
      less
      htop
      curl
      man
      git
      gitAndTools.diff-so-fancy
      openssl
      pkgconfig
      perl
      nixpkgs-fmt
      python37
      python37Packages.psycopg2
      python37Packages.pip
      python37Packages.virtualenv
      python37Packages.pre-commit
      cacert
    ] ++ (
      stdenv.lib.optionals stdenv.isDarwin [
        frameworks.Security
        frameworks.CoreServices
        frameworks.CoreFoundation
        frameworks.Foundation
      ]
    );

    # ENV Variables
    RUST_BACKTRACE = 1;
    HISTFILE = "${toString ./.}/.zsh_history";
    SOURCE_DATE_EPOCH = 315532800;
    LIBCLANG_PATH = "${llvmPackages.libclang}/lib";
    PROJDIR = "${toString ./.}";

    # Post Shell Hook
    shellHook = ''
      echo "Using ${python37.name}, ${rust.name}"
      echo "ENV: graft-env activated";
    '';
  }
