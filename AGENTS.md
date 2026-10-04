# AGENTS.md

Rust (gtk4-rs + libadwaita) port of Vocalis / gnome-sound-recorder. `src/*.rs` mirror the original
GJS modules. Authoritative plan/status: `docs/rust-port-plan.md`. User-facing docs: `README.md`.

## Commands

The system lacks GTK/`pkg-config`; always run through the flake dev shell:

- `nix develop -c cargo run` (or enter `nix develop`, then `cargo …`)
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings` (CI enforces `-D warnings`)
- `cargo test` — runs **zero** tests; the only test is `#[ignore]`d.
  Run it with `cargo test -- --ignored` (`recorder::tests::records_to_file`; needs a capture
  device, records ~1.5 s, then deletes the file).
- `nix build` / `nix run .#` — the packaged app; `nix profile install .#` is needed for the
  desktop/titlebar icon to resolve on the desktop.
- `blueprint-compiler compile data/ui/<name>.blp` / `… decompile data/ui/<name>.ui` / `… format`
  — author UI in `.blp`; `build.rs` compiles it to `.ui` in `$OUT_DIR` before `glib-compile-resources`.

## Nix gotchas

- Flakes read the **git tree**: untracked files are invisible to `nix build .#` / `nix develop .#`.
  `git add` new files, or use `nix develop "path:."` (copies the working tree).
- `flake.nix` env changes require **re-entering** the shell; the current shell keeps the old env.
- The dev shell must expose GTK4's own GSettings schemas (in `GSETTINGS_SCHEMA_DIR`) or file
  dialogs abort with `org.gtk.gtk4.Settings.FileChooser … not installed`; `adwaita-icon-theme` is
  needed for themed symbolic icons.

## Architecture (non-obvious)

- The GResource is embedded at compile time by `build.rs` (`glib-build-tools`); asset manifest is
  `data/app.drey.Vocalis.data.gresource.xml`. The Meson layer must **not** compile resources.
- UI templates are authored as **Blueprint `.blp`** (`data/ui/*.blp`). `build.rs` runs
  `blueprint-compiler batch-compile` into `$OUT_DIR/ui`, then passes `[$OUT_DIR, "data"]` to
  `glib-compile-resources` so the generated `.ui` shadows the source dir. The gresource XML keeps
  referencing `ui/*.ui` (the compiler's output name).
- `RecordingList` is a plain struct owning a `gio::ListStore`, not a `Gio.ListStore` subclass
  (gio-rs cannot subclass it).
- `recording.*` actions live in each `Row`'s action map; `RecordingsListWidget::sync_window_actions`
  hoists the **active row's** group onto the window so app accelerators resolve. `space`/`Delete`
  are window actions (`win.play-pause`, `win.delete-recording`) that dispatch by window state —
  GTK stops at the first matching action **regardless of enabled state**, so never bind those keys
  to both `recorder.*` and `recording.*`.
- `Recorder::stop()` returns `Option<Recording>`; `RecorderWidget`'s `stopped` signal carries it.
- App id is compile-time: `cfg!(feature = "development")` → `app.drey.Vocalis.Devel`. Runtime
  gettext dir/version come from `VOCALIS_LOCALEDIR` / `VOCALIS_VERSION_SUFFIX` (set by Meson).
- `.blp` templates use `Vocalis*` GType names and camelCase ids bound via `#[template_child(id = "…")]`.
- `window.blp` is the composition entrypoint: it names the leaves directly —
  `$VocalisRecorderWidget` as the `recorder` `StackPage`, and `$VocalisRecordingsListWidget` (whose
  `Gtk.ListBox` container lives in `recordings-list.blp`) inside the `Adw.Clamp`. `Window::new`
  registers both GTypes (`static_type()`) before building the template, or GtkBuilder cannot resolve
  them. Per-item `Row`s (`Gtk.ListBox::bind_model`) and the custom `WaveForm` are still built in code.
- `Application::startup` clears the legacy `gtk-application-prefer-dark-theme` flag and sets
  `AdwStyleManager:color-scheme`; `waveform.rs` reads accent/high-contrast from `AdwStyleManager`.
- Keep GStreamer (gstreamer-rs) and the GLib main loop; do **not** introduce `tokio` or replace
  GStreamer with native Rust DSP (see the plan's non-goals).

## Translations

- Source strings use `gettextrs::gettext` / `ngettext`; `po/meson.build` passes
  `--keyword=gettextrs::gettext` / `--keyword=gettextrs::ngettext:1,2`. Keep `ngettext` calls
  **inline with literal msgids** — a helper hides them from `xgettext`.
- `po/POTFILES.in` lists `.rs`, `.blp`, and `data/*.in.in`. Compile/merge targets (Meson layer):
  `meson setup build-meson && meson compile -C build-meson app.drey.Vocalis-pot` and
  `…-update-po`. Don't run `-update-po` casually — the `.po` data is inherited and precious.
- `po/*.pot` and `build-meson/` are gitignored.

## Build/packaging notes

- Primary path is Cargo + Nix. The thin Meson layer (`meson.build`, `src/meson.build`,
  `data/meson.build`, `po/meson.build`) drives the same Cargo build for i18n/install/Flatpak/CI.
- `build-aux/app.drey.Vocalis.Devel.json` is a **development** Flatpak manifest (network build via
  `--share=network`). Flathub forbids network builds and needs vendored `cargo-sources.json`
  (see `docs/rust-port-plan.md`).
- CI (`.gitlab-ci.yml` + `.gitlab-ci/*.yml`) is GitLab-specific (GNOME citemplates).
