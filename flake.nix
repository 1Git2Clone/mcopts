{
  description = "mcopts - a library for managing your Minecraft options";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    git-hooks.url = "github:cachix/git-hooks.nix";
    git-hooks.inputs.nixpkgs.follows = "nixpkgs";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    {
      self,
      nixpkgs,
      git-hooks,
      systems,
      ...
    }:
    let
      forAll = nixpkgs.lib.genAttrs (import systems);
    in
    {
      packages = forAll (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "mcopts";
            version = "0.1.0";
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./build.rs
                ./src
                ./data
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            meta.mainProgram = "mcopts";
          };
        }
      );

      checks = forAll (system: {
        pre-commit-check = git-hooks.lib.${system}.run {
          src = ./.;
          settings.rust.check.cargoDeps = nixpkgs.legacyPackages.${system}.rustPlatform.importCargoLock {
            lockFile = ./Cargo.lock;
          };
          hooks = {
            nixfmt.enable = true;
            rustfmt.enable = true;
            clippy.enable = true;
            shellcheck.enable = true;
          };
        };
      });

      devShells = forAll (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          pre-commit-check = self.checks.${system}.pre-commit-check;
        in
        {
          default = pkgs.mkShell {
            inherit (pre-commit-check) shellHook;
            packages = pre-commit-check.enabledPackages ++ [
              pkgs.cargo
              pkgs.rustc
              pkgs.clippy
              pkgs.rustfmt
            ];
          };

          # game/: real Minecraft clients, offline, on a virtual display. GLFW
          # and OpenAL load these libraries at run time; Mojang's own Java does
          # not run on NixOS, so the game gets nixpkgs' (21 for 1.21.x, 25 for
          # 26.x).
          game = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              # Unwrapped: the wrapper puts the host's /run/opengl-driver
              # first, whose Mesa needs the host's glibc, not this one.
              pkgs.portablemc.unwrapped
              pkgs.xvfb-run
              (pkgs.python3.withPackages (ps: [ ps.xlib ]))
            ];
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
              pkgs.libGL
              pkgs.libglvnd
              pkgs.mesa
              pkgs.libx11
              pkgs.libxext
              pkgs.libxcursor
              pkgs.libxrandr
              pkgs.libxxf86vm
              pkgs.libxi
              pkgs.openal
              pkgs.alsa-lib
              pkgs.libpulseaudio
              pkgs.udev
              pkgs.flite
            ];
            # Xvfb's GLX would otherwise load the host's DRI driver too.
            LIBGL_DRIVERS_PATH = "${pkgs.mesa}/lib/dri";
            # SDL in 26.3 uses EGL on Xvfb; keep its vendor on this Mesa too.
            __EGL_VENDOR_LIBRARY_FILENAMES = "${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
            JDK21 = "${pkgs.jdk21}/bin/java";
            JDK25 = "${pkgs.jdk25}/bin/java";
          };
        }
      );
    };
}
