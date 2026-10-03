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

      appId = "app.drey.Vocalis";

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
          adwaita-icon-theme
          hicolor-icon-theme
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
              meson
              ninja
              pkg-config
              gobject-introspection
              libxml2
              gettext
              desktop-file-utils
              appstream
            ] ++ mkRuntimeDeps pkgs;

            shellHook = ''
              export GSETTINGS_SCHEMA_DIR="$PWD/build/schemas:${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}/glib-2.0/schemas:${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}/glib-2.0/schemas''${GSETTINGS_SCHEMA_DIR:+:$GSETTINGS_SCHEMA_DIR}"
              mkdir -p build/schemas
              sed \
                -e 's/@app-id@/${appId}/g' \
                -e 's/@gettext-package@/${appId}/g' \
                data/app.drey.Vocalis.gschema.xml.in \
                > build/schemas/${appId}.gschema.xml
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
            doCheck = false;

            nativeBuildInputs = with pkgs; [
              pkg-config
              wrapGAppsHook4
              gobject-introspection
              glib
              libxml2
              gettext
              desktop-file-utils
              gtk4
            ];

            buildInputs = mkRuntimeDeps pkgs;

            postInstall = ''
              runHook prePostInstall

              # GSettings schema
              mkdir -p $out/share/glib-2.0/schemas
              sed \
                -e 's/@app-id@/${appId}/g' \
                -e 's/@gettext-package@/${appId}/g' \
                data/app.drey.Vocalis.gschema.xml.in \
                > $out/share/glib-2.0/schemas/${appId}.gschema.xml
              glib-compile-schemas $out/share/glib-2.0/schemas

              # Icons
              install -Dm644 data/icons/app.drey.Vocalis.svg \
                $out/share/icons/hicolor/scalable/apps/${appId}.svg
              install -Dm644 data/icons/app.drey.Vocalis-symbolic.svg \
                $out/share/icons/hicolor/symbolic/apps/${appId}-symbolic.svg

              # .desktop (translation-aware, with fallback)
              desktopTemplate="$TMPDIR/${appId}.desktop"
              sed "s/@app-id@/${appId}/g" data/app.drey.Vocalis.desktop.in.in > "$desktopTemplate"
              install -d $out/share/applications
              msgfmt --desktop --template="$desktopTemplate" -d po \
                -o $out/share/applications/${appId}.desktop \
                || install -Dm644 "$desktopTemplate" $out/share/applications/${appId}.desktop

              # metainfo (translation-aware, with fallback)
              metainfoTemplate="$TMPDIR/${appId}.metainfo.xml"
              sed \
                -e 's/@app-id@/${appId}/g' \
                -e 's/@gettext-package@/${appId}/g' \
                data/app.drey.Vocalis.metainfo.xml.in.in > "$metainfoTemplate"
              install -d $out/share/metainfo
              msgfmt --xml --template="$metainfoTemplate" -d po \
                -o $out/share/metainfo/${appId}.metainfo.xml \
                || install -Dm644 "$metainfoTemplate" $out/share/metainfo/${appId}.metainfo.xml

              # Translations
              for po in po/*.po; do
                lang=$(basename "$po" .po)
                install -d $out/share/locale/$lang/LC_MESSAGES
                msgfmt "$po" -o $out/share/locale/$lang/LC_MESSAGES/${appId}.mo
              done

              runHook postPostInstall
            '';

            preFixup = ''
              # Point gettext at the packaged .mo files (read by main.rs).
              gappsWrapperArgs+=(--set VOCALIS_LOCALEDIR "$out/share/locale")

              # Make the icon themes discoverable at runtime (GTK's default theme
              # is "Adwaita"; without it, themed icons like media-record-symbolic
              # don't resolve). wrapGAppsHook only forwards gsettings-schemas dirs.
              gappsWrapperArgs+=(--prefix XDG_DATA_DIRS : "${pkgs.adwaita-icon-theme}/share")
              gappsWrapperArgs+=(--prefix XDG_DATA_DIRS : "${pkgs.hicolor-icon-theme}/share")

              gtk4-update-icon-cache -q -t -f $out/share/icons/hicolor || true
              update-desktop-database -q $out/share/applications || true
            '';

            meta = with pkgs.lib; {
              description = "A simple, modern sound recorder for GNOME";
              homepage = "https://gitlab.gnome.org/World/vocalis";
              license = licenses.gpl3Plus;
              mainProgram = "vocalis";
              platforms = platforms.linux;
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
