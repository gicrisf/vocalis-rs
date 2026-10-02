{
  description = "Vocalis — a simple, modern sound recorder for GNOME (gtk4-rs port)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);

      mkRuntimeDeps =
        pkgs: with pkgs; [
          gtk4
          libadwaita
          glib
          gst_all_1.gstreamer
          gst_all_1.gst-plugins-base
          gst_all_1.gst-plugins-good
          gst_all_1.gst-plugins-bad
          gst_all_1.gst-plugins-ugly
          gsettings-desktop-schemas
        ];
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              rust-analyzer
              clippy
              rustfmt
              pkg-config
              gobject-introspection
              libxml2
              gettext
              desktop-file-utils
              appstream
            ] ++ mkRuntimeDeps pkgs;

            shellHook = ''
              export GSETTINGS_SCHEMA_DIR="$PWD/build/schemas:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}$GSETTINGS_SCHEMA_DIR"
              mkdir -p build/schemas
              sed \
                -e 's/@app-id@/app.drey.Vocalis/g' \
                -e 's/@gettext-package@/app.drey.Vocalis/g' \
                data/app.drey.Vocalis.gschema.xml.in \
                > build/schemas/app.drey.Vocalis.gschema.xml
              glib-compile-schemas --targetdir build/schemas build/schemas || true

              echo "Vocalis dev shell: cargo build / cargo run"
            '';
          };
        }
      );

      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "vocalis";
            version = "1.0.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [
              pkg-config
              wrapGAppsHook4
              gobject-introspection
              glib
              libxml2
            ];

            buildInputs = mkRuntimeDeps pkgs;

            meta = with pkgs.lib; {
              description = "A simple, modern sound recorder for GNOME";
              homepage = "https://gitlab.gnome.org/World/vocalis";
              license = licenses.gpl3Plus;
              mainProgram = "vocalis";
            };
          };
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/vocalis";
        };
      });
    };
}
