# Vocalis

A simple, modern sound recorder for GNOME.

**This is a Rust port of Vocalis** (originally a GJS/TypeScript app), built with
[`gtk4-rs`](https://gtk-rs.org/) and [`libadwaita`](https://gitlab.gnome.org/World/Rust/libadwaita-rs).
The port is functionally complete.

![screenshot](data/resources/screenshots/screenshot1.png)

### Quick start (NixOS)

```sh
nix run .#                 # run the packaged app
nix build                  # build → result/bin/vocalis
nix profile install .#     # install (also makes the desktop/titlebar icon resolvable)
```

### Hacking

Development uses the Nix flake for the full toolchain (Rust, GTK4, libadwaita, GStreamer,
GSettings schemas, and an icon theme):

```sh
nix develop                # enter the dev shell
cargo run                  # build and run
cargo fmt --check
cargo clippy --all-targets
cargo test                 # note: the recorder integration test is #[ignore] (needs a mic)
```

Requirements (all provided by `nix develop`): Rust, GTK ≥ 4.16, libadwaita ≥ 1.6,
GStreamer ≥ 1.20. Linux only.

#### Profiles

- `default` → application id `app.drey.Vocalis`
- `development` → application id `app.drey.Vocalis.Devel` (Cargo: `--features development`)

They use separate GSettings and XDG data/cache directories, so both can run side by side.

### Translations, Flatpak and CI

A thin Meson layer drives the same Cargo build and handles i18n, installation, and the
Flatpak/CI integration:

```sh
nix develop
meson setup build-meson
meson compile -C build-meson                          # builds via cargo + compiles translations
meson install -C build-meson                          # binary, schema, desktop, icons, locales
meson compile -C build-meson app.drey.Vocalis-pot        # regenerate the .pot
meson compile -C build-meson app.drey.Vocalis-update-po  # merge into po/*.po
```

Flatpak (currently targets the GNOME nightly runtime; not validated yet):

```sh
flatpak-builder --user --install --force-clean build-flatpak \
  build-aux/app.drey.Vocalis.Devel.json
```

CI: `.gitlab-ci.yml` (Cargo checks + GNOME Flatpak CI templates).

### Links

- Upstream (GJS original): https://gitlab.gnome.org/World/vocalis
- Translations are inherited from upstream: https://wiki.gnome.org/TranslationProject
- Matrix (upstream): [#sound-recorder:gnome.org](https://matrix.to/#/#sound-recorder:gnome.org)

For this fork, report issues in your own tracker.

### License

GPL-3.0-or-later. Derived from the original Vocalis; see [`COPYING`](COPYING) and
[`AUTHORS`](AUTHORS).
