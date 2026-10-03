# Vocalis

A simple, modern sound recorder for GNOME

<a href="https://flathub.org/apps/details/org.gnome.SoundRecorder">
<img src="https://flathub.org/assets/badges/flathub-badge-i-en.png" width="190px" />
</a>

<div align="center">
![screenshot](data/resources/screenshots/screenshot1.png)
</div>

### Useful links

Report issues: https://gitlab.gnome.org/World/vocalis/issues/

Translate: https://wiki.gnome.org/TranslationProject

You can also join us in [#sound-recorder:gnome.org on Matrix](https://matrix.to/#/#sound-recorder:gnome.org)

### Hacking on Vocalis

This is a Rust (`gtk4-rs` / `libadwaita`) port of Vocalis. Development uses the Nix flake:

```sh
nix develop              # toolchain + GTK/Adw/GStreamer + GSettings schemas + icon theme
cargo run                # build and run
cargo clippy --all-targets
cargo test               # note: the recorder integration test is #[ignore] (needs a mic)
nix build                # build the wrapped package → result/bin/vocalis
nix run .#               # run the packaged app
```

See [`docs/rust-port-plan.md`](docs/rust-port-plan.md) for the port plan and status.

### Translations, Flatpak and CI

A thin Meson layer drives the same Cargo build and handles i18n, installation and the
Flatpak/CI integration:

```sh
nix develop
meson setup build-meson
meson compile -C build-meson                          # builds via cargo + compiles translations
meson install -C build-meson                          # binary, schema, desktop, icons, locales
meson compile -C build-meson app.drey.Vocalis-pot        # regenerate the .pot
meson compile -C build-meson app.drey.Vocalis-update-po  # merge into po/*.po
```

- Flatpak manifest: `build-aux/app.drey.Vocalis.Devel.json` (`buildsystem: meson`).
- CI: `.gitlab-ci.yml` (cargo checks + GNOME Flatpak CI templates).

