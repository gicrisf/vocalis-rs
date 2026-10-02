# Vocalis → Rust (gtk4-rs) Port Plan

Status: planning
App: Vocalis (`app.drey.Vocalis`), a GNOME sound recorder (formerly gnome-sound-recorder)
Current implementation: GJS + TypeScript, ~1,795 non-comment LOC across 11 modules (2,256 total),
plus 658 lines of reusable `.ui` templates.

This document is the authoritative plan for a **faithful 1:1 port to Rust using `gtk4-rs` /
`libadwaita-rs`**, keeping the GObject subclass architecture. A separate appendix records the
**lean port** options so we can improve incrementally in later phases.

---

## 1. Objective and guiding principles

1. **Behavior parity first.** Every feature, action, signal, property, and shortcut in the current
   TypeScript app must exist and behave identically in Rust. No feature work during the port.
2. **Faithful architecture.** Keep GObject subclassing, GObject properties, custom signals,
   `Gio.ListStore`, action groups, and C-style Gio async callbacks. Do not restructure into
   plain structs, `BoxedAnyObject`, or `relm4` during this port.
3. **Maximize reuse.** The `.ui` templates, `style.css`, icons, `gschema`, `.desktop`, metainfo,
   and `po/` translations are reused unchanged. This is a large share of the project.
4. **Stay on the GLib main loop.** No `tokio`. GTK/Adw/Gio are main-thread only; async is done with
   `glib`'s futures / `gio`'s `async` feature on the `MainContext`.
5. **Packaging parity.** Keep Flatpak/CI building. Preserve the `default`/`development` profile
   split (`.Devel` app id).
6. **NixOS-first dev.** Provide a `flake.nix` with a devShell (and package) pinning the exact
   GTK/GStreamer/Adw versions.

### Non-goals (explicitly deferred)

- The sub-segment/trim feature and JSON metadata sidecar (separate plan; see §13 note).
- Any UI/HIG redesign.
- Replacing GStreamer with native Rust DSP.
- Replacing GObject models with plain structs (that is the lean-port appendix).

---

## 2. Current inventory

### 2.1 TypeScript modules

| File | code LOC | Role |
|---|---:|---|
| `src/application.ts` | 150 | `Adw.Application` subclass, actions, accels, dirs, settings, gst init, About |
| `src/main.ts` | 5 | Entry point (`app.runAsync`) |
| `src/recorder.ts` | 274 | GObject: GStreamer record pipeline, level→peak, duration |
| `src/recorderWidget.ts` | 183 | `Gtk.Box` subclass: recording UI + `recorder.*` actions |
| `src/recording.ts` | 218 | GObject: one file, duration via Discoverer, peaks cache, trash/copy |
| `src/recordingList.ts` | 99 | `Gio.ListStore` subclass: dir enumeration + monitor + sorted insert |
| `src/recordingListWidget.ts` | 156 | `Adw.Bin` subclass: `Gtk.ListBox` model binding, `GstPlayer.Player` |
| `src/row.ts` | 293 | `Gtk.ListBoxRow` subclass: playlist row, `recording.*` actions |
| `src/utils.ts` | 44 | Time/date formatting (gettext markup) |
| `src/waveform.ts` | 174 | `Gtk.DrawingArea` subclass: cairo waveform + drag seek |
| `src/window.ts` | 199 | `Adw.ApplicationWindow` subclass: state machine, undo toasts |
| **total** | **1,795** | |

### 2.2 Reused assets (unchanged)

- `data/ui/{window,recorder,row,help-overlay}.ui` (658 lines)
- `data/style.css`, `data/icons/*`, `data/resources/screenshots/*`
- `data/app.drey.Vocalis.gschema.xml.in`, `.desktop.in.in`, `.metainfo.xml.in.in`
- `po/` translations and `po/POTFILES.in`
- Flatpak manifest and CI scaffolding

### 2.3 Things that disappear

- `src/app.drey.Vocalis.src.gresource.xml.in` (JS module bundle) — no JS resources in Rust.
- `src/app.drey.Vocalis.in` (GJS launcher) — replaced by the compiled binary.
- `types/` ambient/gettext shims, `gi-types/`, `package.json`, `package-lock.json`, `tsconfig.json`,
  `.eslintrc.js`, and `tsc`/npm toolchain.
- All `Gio._promisify(...)` calls (application.ts:74-89) become unnecessary.

---

## 3. Target stack

Pin exact versions in the flake. The only hard constraint is that `gtk4-rs` and `libadwaita-rs`
must be version-compatible with each other and expose the Adw APIs used (notably
`AdwStyleManager::accent-color` and `high-contrast`, which require libadwaita ≥ 1.6; the current
build already requires `libadwaita-1 >= 1.6.alpha`, meson.build:45).

| Crate | Purpose | Notes |
|---|---|---|
| `gtk4` | GTK4 widgets | feature set must cover `v4_14`+; match libadwaita crate |
| `libadwaita` | Adw widgets | enable `v1_6` for accent-color/high-contrast |
| `glib` | main loop, subclassing, `clone!`, `BoxedAnyObject`, futures | features: `v2_80`+, `clone-rs-macros` |
| `gio` | files, settings, actions, monitors, `ListStore` | enable `v2_80`+; `async` support |
| `gstreamer` | pipeline, bus, caps, tags | |
| `gstreamer-player` | `gst::Player` playback | |
| `gstreamer-pbutils` | `Discoverer`, encoding profiles | |
| `cairo-rs` | waveform drawing | |
| `serde` + `serde_json` | JSON (peaks cache now; trim metadata later) | |
| `gettext-rs` | `_()` / `ngettext()` | |
| `anyhow` / `thiserror` | error propagation in non-GObject code | |
| `once_cell` | `RecordingsDir`/`CacheDir`/`Settings` globals | or `std::sync::OnceLock` |

**Not used:** `tokio`, any non-GTK GUI toolkit, any Rust-native audio stack. See §14 for why.

---

## 4. Source layout

```
Cargo.toml
build.rs                     # compile data gresource via glib-build-tools
flake.nix                    # devShell + package (NixOS)
src/
  main.rs                    # was main.ts
  config.rs                  # was pkg.* globals + application.ts exports
  application.rs             # was application.ts
  window.rs                  # was window.ts
  recorder.rs                # was recorder.ts
  recorder_widget.rs         # was recorderWidget.ts
  recording.rs               # was recording.ts
  recording_list.rs          # was recordingList.ts
  recording_list_widget.rs   # was recordingListWidget.ts
  row.rs                     # was row.ts
  waveform.rs                # was waveform.ts
  utils.rs                   # was utils.ts
data/                        # unchanged .ui, css, icons, schema, desktop, metainfo
po/                          # unchanged
```

Module wiring via `mod` declarations in `main.rs` (flat module tree, matching the TS file layout).

---

## 5. GJS → gtk4-rs idiom mapping

| GJS / TypeScript | Rust (gtk4-rs) |
|---|---|
| `class X extends GObject.Object` + `GObject.registerClass({...}, this)` | `glib::wrapper! { pub struct X(ObjectSubclass<imp::X>); }` + `#[glib::object_subclass]` |
| `Template: "resource:///..."` + `InternalChildren` | `#[derive(CompositeTemplate)]` + `#[template(resource = "...")]` + `#[template_child]` |
| `Signals: { foo: {param_types: [...]} }` | `glib::subclass::Signal::builder("foo", &[...], ret).build()` |
| `GObject.ParamSpec.int/float/string/bool` | `#[derive(Properties)]` + `glib::ParamSpecInt/Float/String/Boolean` + `derived_properties` |
| `obj.bind_property(a, b, flags)` | `ObjectExt::bind_property(a, b, flags)` |
| `new Gio.SimpleAction({...})` + `connect("activate")` | `gio::SimpleAction::new(...)` + `connect_activate` |
| `widget.insert_action_group("x", group)` | `WidgetExt::insert_action_group("x", &group)` |
| `Gst.ElementFactory.make` | `gstreamer::ElementFactory::make("...").build()?` |
| `Gst.parse_launch(...)` | `gstreamer::parse::launch(...)?` / `parse_launch_full` |
| `GLib.timeout_add` | `glib::timeout_add_local(...)` |
| `Gio._promisify(...)` + `await` | `gio` async future + `glib::MainContext::spawn_local` |
| `TextEncoder` + `JSON.stringify` | `serde_json::to_vec` / `from_slice` |
| `_()` / `Gettext.ngettext(...)` | `gettextrs::gettext!` / `gettextrs::ngettext!` |
| `pkg.name` / `pkg.version` | `config::APP_ID` / `env!("CARGO_PKG_VERSION")` |
| `GObject.ValueArray` peak read | `structure.get::<glib::ValueArray>("peak")` |
| `styleManager.accent_color` | `adw::StyleManager::default().accent_color_rgba()` |
| `Adw.accent_color_to_standalone_rgba` | `adw::accent_color_to_standalone_rgba(...)` |

---

## 6. Module-by-module plan

Estimated lines are for the faithful port. Overall target: **~2,900–3,400 Rust LOC** plus
`Cargo.toml`/`build.rs`.

### 6.1 `main.rs` (~20)
- `fn main() -> glib::ExitCode`; `gst::init()` happens later in `Application::startup` (parity
  with application.ts:149), not here.
- Instantiate `Application` and `run()` / `run_with_args`.
- Port `main.ts` `app.runAsync(argv)` semantics (CLI argc/argv passed through).

### 6.2 `config.rs` (~60)
Replaces the `pkg.*` GJS globals and the module-level exports in application.ts:28-34.
- `APP_ID`: `app.drey.Vocalis` or `app.drey.Vocalis.Devel`, chosen at compile time by a Cargo
  feature `development` (mirrors `-Dprofile=development`, meson.build:9-24).
- `RESOURCE_BASE_PATH = "/app/drey/Vocalis/"` (constant; application.ts:48).
- `VERSION` from `env!("CARGO_PKG_VERSION")`, plus an optional `-devel` suffix supplied by
  `build.rs` from `VOCALIS_VERSION_SUFFIX` (set by the flake/CI, e.g. from `git rev-parse --short`).
- Lazy globals via `once_cell`:
  - `recordings_dir()` = `glib::user_data_dir()/APP_ID`
  - `cache_dir()` = `glib::user_cache_dir()/APP_ID`
  - `settings()` = `gio::Settings::new(APP_ID)`
- The `RecordingsDir`/`CacheDir` objects are created after `startup` in TS; in Rust these can be
  lazy statics.

### 6.3 `application.rs` (~200) — `AdwApplication`
Source: application.ts.
- `glib::wrapper!` for `Application` extending `adw::Application`, `#[glib::object_subclass]`.
- Constructor: `application_id`, `resource_base_path`, `glib::set_application_name`.
- Keep env vars: `PULSE_PROP_media.role=production`, `PULSE_PROP_application.icon_name` (application.ts:51-52).
- `--version` main option: `add_main_option` + `handle-local-options` (application.ts:54-72).
- `startup`: `gst::init()`, `init_user_directory(cache_dir)`, `init_user_directory(recordings_dir)`,
  `init_app_menu()`.
- Actions: `app.audio-profile`, `app.audio-channel` from `gio::Settings` (`Settings::create_action`
  if available in the pinned `gio`; otherwise a thin `gio::SimpleAction` wrapper updating the
  setting), `app.about`, `app.quit`.
- Accelerators (`set_accels_for_action`, application.ts:111-128): port verbatim, including
  `win.show-help-overlay` (provided automatically by `AdwApplicationWindow`).
- `activate`: create `Window`, add `.devel` CSS class when `APP_ID` ends with `Devel`
  (application.ts:160).
- `AboutDialog` with all artists/developers/credits/license/website/issue URL (application.ts:165-195).
  Use `adw::AboutDialog`.

### 6.4 `utils.rs` (~80)
Source: utils.ts.
- `format_time(ns: u64) -> String` returning Pango markup (`<small>` millis). **Preserve current
  output** but note the JS `Date`-based implementation (utils.ts:24-34) is wrong for durations
  ≥ 24h (`Date` wraps); decide during port whether to fix or replicate. Plan: fix to integer
  arithmetic (no behavioral regression for normal durations).
- `display_date_time(time: &glib::DateTime) -> String` using `glib::DateTime` difference and
  `gettextrs::ngettext!`. Port thresholds verbatim (utils.ts:36-72).

### 6.5 `recording.rs` (~350) — `Recording` (`glib::Object`)
Source: recording.ts. This is a data model, not a widget; subclass faithfully.
- Properties: `duration` (int), `name` (string).
- Signals: `peaks-updated`, `peaks-loading`.
- Constructor reads `query_info("time::created,time::modified,standard::content-type")`, maps
  content-type → extension via `EncodingProfiles` (recording.ts:65-77).
- Duration: `gst_pbutils::Discoverer` started, `discovered` signal sets `duration`, then
  `discover_uri_async(uri)`. Keep the discoverer alive (store it or tie lifetime to the object).
- Peaks: `load_peaks()` async (reads JSON cache from `cache_dir()/<name>_data`), `generate_peaks()`
  builds the `uridecodebin ! audioconvert ! audio/x-raw,channels=1 ! level ! fakesink` pipeline
  and collects `level` peaks on EOS (recording.ts:212-251). Port `TextDecoder`/`JSON.parse` to
  `serde_json`; add a `Vec<f32>` deserializer with a validity check (mirrors `isNumArray`).
- `delete()`: `trash_async` on file and peaks cache.
- `save(dest)`: `copy_async`.
- Rename: `set name` → `set_display_name` + notify.
- Note: `duration` ParamSpec max is `GLib.MAXINT16` (recording.ts:41, recorder.ts:106) which is
  wrong for nanosecond values. In Rust use `i64`/`u64` properties; do **not** reproduce the bug.

### 6.6 `recorder.rs` (~430) — `Recorder` (`glib::Object`)
Source: recorder.ts. The GStreamer record engine.
- `EncodingProfiles` const table (recorder.ts:31-71) and `AudioChannels` table (recorder.ts:73-76).
- Properties: `duration` (int64 in Rust), `current-peak` (float, 0..1).
- Constructor builds and links `pulsesrc → audioconvert → level` with `audio/x-raw` caps
  (recorder.ts:124-152). Keep elements in the imp struct.
- `start()`: pick a non-existing `Recording N` display name, attach bus signal watch, set
  `encodebin` profile + `filesink location`, link `level → encodebin → filesink`, set PLAYING,
  add a 100 ms `timeout_add_local` polling `query_position(TIME)` (recorder.ts:154-188).
- `pause`/`resume`/`stop`: state transitions; `stop` removes timeout/watch, builds a `Recording`
  with copied peaks if the file exists (recorder.ts:190-225).
- `get_profile()`: build `gst_pbutils::EncodingAudioProfile` + `EncodingContainerProfile` from
  caps strings and channel count (recorder.ts:276-305). Use `gst::Caps::builder`/`from_str`.
- Bus `message::Element` handler: missing-plugin detail/description logging; `level` structure
  `peak` via `glib::ValueArray`, `current_peak = 10^(peak/20)` (recorder.ts:227-269).
- **Note:** `current_peak` setter has an odd `if (peak > 0) peak = 0;` (recorder.ts:322). Replicate
  to keep parity, or flag as a bug to fix later. Do not silently change behavior.

### 6.7 `recording_list.rs` (~250) — `RecordingList` (`gio::ListStore` subclass)
Source: recordingList.ts. **This is the most awkward Rust subclass** (subclassing `ListStore`).
- `glib::wrapper!` extending `gio::ListStore`; `ListModelImpl` (and `ListStoreImpl` if required).
- Fields: `Cancellable`, `FileMonitor`, optional `FileEnumerator`.
- Constructor: monitor `recordings_dir` with `WATCH_MOVES`; on `changed`, handle
  `MOVED_OUT` (remove by uri) and `MOVED_IN` (sorted insert new `Recording`) (recordingList.ts:27-42).
- `enumerate_children_async` then paged `next_files_async(5)` loop, inserting each as a `Recording`
  sorted by `timeModified` descending (recordingList.ts:44-116).
- **Gotcha:** this store will index every file in the recordings dir; the future metadata sidecar
  work must filter `.json` here. Out of scope for the port.
- Error handling: swallow `CANCELLED`, log others (recordingList.ts:77-82).

### 6.8 `waveform.rs` (~340) — `WaveForm` (`Gtk.DrawingArea`)
Source: waveform.ts. Based on Pitivi.
- Properties: `position` (float 0..1), `peak` (float 0..1); signals `position-changed` (double),
  `gesture-pressed`; ctor param `WaveType { Recorder, Player }`.
- Player mode adds `GtkGestureDrag` with `drag-begin`/`drag-update`/`drag-end`
  (waveform.ts:111-127).
- Connect to `adw::StyleManager` `notify::high-contrast` and `notify::accent-color` to
  `queue_draw` (waveform.ts:100-106).
- `set_draw_func` cairo routine (waveform.ts:128-199): divider color per mode
  (`accent_color_to_standalone_rgba` for Player, `destructive_color` lookup otherwise), LTR/RTL
  peak layout with `GUTTER = 4`, alpha differences.
- Setters/getters for `peak`, `peaks`, `position`, `clamped`.
- `destroy()` disconnects style-manager handlers.
- **Bug to decide:** `set peak` pushes `p.toFixed(2)` (a string) into a `number[]`
  (waveform.ts:206). Use `f32` in Rust; behavior is visually equivalent.

### 6.9 `row.rs` (~450) — `Row` (`Gtk.ListBoxRow`)
Source: row.ts. Per-recording playlist row.
- Template `row.ui` with children: `playbackStack`, `mainStack`, `waveformStack`, `rightStack`,
  `name`, `entry`, `date`, `duration`, `revealer`, `playbackControls`, `saveBtn`, `playBtn`,
  `pauseBtn`.
- Signals: `play(string)`, `pause`, `seek-backward`, `seek-forward`, `deleted`; property `expanded`.
- Constructs a `WaveForm` (Player) and reparents into `waveformStack`; loads peaks or cache.
- Property bindings: `recording.name → name.label`, `recording.name → entry.text`,
  `row.expanded → revealer.reveal_child` (row.ts:114-131).
- Action group `recording` with `export`, `save`, `rename`, `pause`, `play`, `delete`,
  `seek-backward`, `seek-forward` (row.ts:133-227). Note `seek-backward`'s handler is an empty
  stub (row.ts:213-217) — replicate or fix deliberately.
- Export dialog: `Gtk.FileChooserNative` save flow, default name `name.extension`
  (row.ts:135-162).
- Rename flow with error CSS class and Escape-to-cancel key controller (row.ts:233-278).
- Edit mode toggles `mainStack`/`rightStack` and enables/disables all actions except `save`.
- Player state machine `Playing`/`Paused` toggles `playbackStack`, focus, action enablement
  (row.ts:318-335).
- Duration display uses `set_markup(format_time(...))`, forced LTR (row.ts:257-265).

### 6.10 `recorder_widget.rs` (~270) — `RecorderWidget` (`Gtk.Box`)
Source: recorderWidget.ts.
- Template `recorder.ui`; children `recorderBox`, `playbackStack`, `recorderTimeBin`,
  `recorderTime`, `pauseBtn`, `resumeBtn`.
- Signals: `canceled`, `paused`, `resumed`, `started`, `stopped(object)`.
- Prepends the `WaveForm` (Recorder type); binds `recorder.current-peak → waveform.peak`.
- Action group `recorder` with `start`/`pause`/`stop`/`resume`/`cancel`; `start.enabled` inverted
  to `cancel.enabled` (recorderWidget.ts:104-111).
- Cancel dialog: `adw::AlertDialog` with `close`(Resume)/`delete`(Delete) responses; on delete
  stop recorder, trash recording, destroy waveform, emit `canceled` (recorderWidget.ts:138-167).
- State setter enables/disables actions and toggles `paused` CSS class.

### 6.11 `recording_list_widget.rs` (~240) — `RecordingsListWidget` (`Adw.Bin`)
Source: recordingListWidget.ts.
- Builds a `Gtk.ListBox` (`boxed-list`, margins) and binds the model with `bind_model` to create
  `Row`s (recordingListWidget.ts:72-143).
- Owns the `gst_player::Player` and wires `state-changed`, `position-updated` to the active row's
  waveform (recordingListWidget.ts:46-70).
- Per-row wiring: waveform `gesture-pressed` (set uri), `position-changed` (seek), `play`,
  `pause`, `seek-backward`/`seek-forward` (±10 s, bounds-clamped), `deleted` (isolate + emit
  `row-deleted`).
- `row-activated` expands/collapses rows and applies `expanded`/`expanded-before`/`expanded-after`
  CSS classes (recordingListWidget.ts:148-181).
- Signal `row-deleted(object, int)`.

### 6.12 `window.rs` (~310) — `Window` (`Adw.ApplicationWindow`)
Source: window.ts.
- Template `window.ui`; children `mainStack`, `emptyPage`, `column`, `toastOverlay`, `toolbarView`.
- Owns `Recorder`, `RecorderWidget`, `gst_player::Player` (+ `PlayerGMainContextSignalDispatcher`),
  `RecordingList`, `RecordingsListWidget`.
- State machine `Empty`/`List`/`Recorder` drives `mainStack.visible_child_name` and
  `toolbarView.reveal_top_bars` (window.ts:229-249).
- Undo-toast system: `win.undo` action, `Adw.Toast` per deletion, dismiss→trash, undo→reinsert
  (window.ts:200-227, 122-127). Port `toastUndo`, `undoToasts`, `undoSignalID` state.
- `win.open-primary-menu` stateful action toggles the header menu (window.ts:128-136).
- `vfunc_close_request`: dismiss toasts, cancel list cancellable, stop recording pipelines, stop
  recorder (window.ts:155-169).
- Icon name `${APP_ID}`; `.devel` class added by Application.

---

## 7. Async and main-loop strategy

- **Executor:** GLib `MainContext` only. Use `glib::MainContext::spawn_local` (or
  `spawn_future_local`) with `gio`'s `async` feature. No `tokio`.
- **Async file I/O:** `gio::File::load_bytes_async` / `replace_contents_async` / `trash_async` /
  `enumerate_children_async` / `next_files_async` become futures. Replaces all `Gio._promisify`
  (`application.ts:74-89`).
- **Timers:** `recorder`'s 100 ms duration poll → `glib::timeout_add_local`; store the
  `SourceId` and remove on stop.
- **GStreamer bus:** `bus.add_signal_watch()` + `connect_message`, or `bus.add_watch_local`
  (prefer `add_watch_local` to stay on the main context). The `Recording::generate_peaks` and
  `Recorder` both need this.
- **GstPlayer:** `PlayerGMainContextSignalDispatcher::new(None)` + `gst_player::Player::new(None,
  Some(dispatcher))`, exactly as window.ts:89-92.

---

## 8. GStreamer details

- Initialize once in `Application::startup` (`gst::init()`), matching application.ts:149.
- **Recording pipeline:** `pulsesrc ! audioconvert ! level ! encodebin ! filesink`
  (recorder.ts:132-152). `level` gets `audio/x-raw` caps via `link_filtered`.
- **Encoding profiles:** map `AudioProfile` setting enum index → `EncodingProfiles` entry →
  `gst::Caps` (with channel count) → `EncodingAudioProfile` inside `EncodingContainerProfile`
  (recorder.ts:276-305).
- **Peaks generation pipeline:** `uridecodebin ! audioconvert ! audio/x-raw,channels=1 ! level !
  fakesink` (recording.ts:212-251).
- **Peak extraction:** `level` message structure `peak` is a `ValueArray`; take nth(0) and convert
  `10^(db/20)`.
- **Duration units:** nanoseconds everywhere (`Format::TIME`); keep `i64`.
- **Missing plugins:** keep `is_missing_plugin_message` logging (recorder.ts:230-241).
- **Playback:** `gst_player::Player`; `seek(position_ns)`, `position`, `duration`; `PlayerState`
  STOPPED/PLAYING transitions drive row state (recordingListWidget.ts:46-70).

---

## 9. Build, resources, and packaging

### 9.1 Resources

- Keep only the **data** bundle (`data/app.drey.Vocalis.data.gresource.xml`): `style.css`,
  `ui/{recorder,row,window}.ui`, aliased `gtk/help-overlay.ui`, and the two symbolic skip icons.
  Prefixes and paths stay identical so templates resolve.
- Drop the **src/JS** bundle (`src/app.drey.Vocalis.src.gresource.xml.in`).
- In `build.rs`, compile the data bundle with `glib-build-tools` (`glib_build_tools::compile_resources`)
  against the existing XML, then register it in `main.rs` via the generated
  `resources_register_include!` macro. `RESOURCE_BASE_PATH` remains `/app/drey/Vocalis/`.
- `style.css` is auto-loaded by libadwaita from `<resource_base_path>/style.css`; **verify against
  the pinned libadwaita version**, otherwise register a `gtk::CssProvider` for `gtk::gdk::Display`
  at startup. (There is no explicit CSS load in the TS code, so this relies on Adw behavior.)
- `help-overlay.ui` alias at `gtk/help-overlay.ui` enables the built-in
  `win.show-help-overlay` shortcut from `AdwApplicationWindow`.

### 9.2 Build orchestration — Cargo-native (decided)

Decision: **Cargo-native.** The goal is a personal fork with a NixOS flake, not upstream submission,
so the GNOME/Flatpak packaging parity that favored Meson is not worth the tooling cost. Cargo owns
compilation, tests, and the dev loop; the Nix flake owns installation of data and translations.

- **Build:** `cargo build` / `cargo run` / `cargo test` are the source of truth. `build.rs` (using
  `glib-build-tools`) compiles the data gresource automatically, so `cargo run` is sufficient for
  development.
- **Removed:** `meson.build`, `meson_options.txt`, `data/meson.build`, `po/meson.build`,
  `src/meson.build`, and the `tsc`/npm pipeline. `po/` (the `.po` files and `POTFILES.in`) stays as
  translation *data*; it is compiled by the flake/CI, not at runtime.
- **Install logic moves to the flake package (and optionally a small `Makefile`/`justfile` for
  non-Nix installs):**
  - binary → `$out/bin/vocalis`
  - `data/ui/*.ui`, `style.css`, `data/icons/*` (icon theme dir, renamed to `APP_ID`)
  - `.desktop`, `.metainfo.xml` → standard XDG dirs
  - `gschema.xml` → `$out/share/glib-2.0/schemas` + `glib-compile-schemas`
  - locale: `msgfmt` each `po/*.po` → `$out/share/locale/<lang>/LC_MESSAGES/<APP_ID>.mo`
    (replaces `i18n.gettext`/`merge_file`)
  - caches: `glib-compile-schemas`, `gtk4-update-icon-cache`, `update-desktop-database` in the
    package fixup phase.

**Dev-loop caveats (Cargo-native):**

- **GSettings in dev:** `gio::Settings::new(APP_ID)` needs a compiled schema on the search path.
  Provide a `make schemas` target (or a flake `shellHook`) that compiles `data/*.gschema.xml` into a
  local dir and export `GSETTINGS_SCHEMA_DIR` so `cargo run` works uninstalled.
- **Translations in dev:** uninstalled runs show source strings. For testing a locale, compile
  `.mo` files into a local tree and have `main.rs` call `gettextrs::bindtextdomain(APP_ID, dir)`
  with `dir` taken from `VOCALIS_LOCALEDIR` when set (falls back to the system locale dir).
- Optionally add a **thin Meson layer later, only if you decide to upstream**; it is not part of
  this plan's build path.

### 9.3 Profiles and IDs

- `default` → `app.drey.Vocalis`; `development` → `app.drey.Vocalis.Devel`, with `-devel`
  version suffix from git short hash (meson.build:9-22, 31).
- Cargo feature `development` selects `APP_ID`; the `-devel` version suffix comes from
  `VOCALIS_VERSION_SUFFIX` (set by the flake/CI, e.g. `git rev-parse --short HEAD`).
- Icon names and XDG `data`/`cache` dirs use `APP_ID` (application.ts:28-34, 82, 152).

### 9.4 Flatpak / CI

The existing Flatpak/CI is upstream-oriented and may be dropped for a personal fork. If a Flatpak
is still wanted:

- Remove node20 + typescript SDK extensions and `append-path` (manifest lines 6-8, 21-23;
  `.gitlab-ci.yml:17,22`).
- Switch the module to `buildsystem: simple` and drive `cargo build --release`, or use the
  `org.freedesktop.Sdk.Extension.rust-stable` SDK extension. Vendor dependencies for reproducible
  offline builds via `cargo-sources.json` (generated with `flatpak-cargo-generator.py`).
- Install data/locale/schema as in §9.2 inside the module's `install` commands.
- Replace the `eslint` CI job (`.gitlab-ci.yml:27-38`) with `cargo fmt --check`, `cargo clippy`,
  and `cargo test`. If the fork has no CI, this is optional.

### 9.5 Nix flake (NixOS-first)

- **Inputs:** pin `nixpkgs` to a revision whose `gtk4`/`libadwaita` satisfy §3 (libadwaita ≥ 1.6
  for accent-color/high-contrast); optionally `flake-utils`.
- **`devShells.default`** provides the build/runtime closure:
  - `rustc`, `cargo`, `rustfmt`, `clippy`, `rust-analyzer`, `pkg-config`
  - `gtk4`, `libadwaita`, `glib`, `gst_all_1.gstreamer` (provides `gstreamer`,
    `gstreamer-player`, `gstreamer-pbutils`), `gst_all_1.gst-plugins-base/good/bad/ugly`, `cairo`
  - build-time: `gobject-introspection` (typelibs for the `-sys` crates)
  - tooling: `gettext`, `desktop-file-utils`, `appstream`, `glib` (`glib-compile-schemas`),
    `gtk4` (`gtk4-update-icon-cache`)
  - `shellHook`: compile `data/*.gschema.xml` to a local dir and export `GSETTINGS_SCHEMA_DIR`,
    so `cargo run` works uninstalled.
- **`packages.default`:** `rustPlatform.buildRustPackage` with `cargoLock.lockFile = ./Cargo.lock`;
  `nativeBuildInputs = [ pkg-config wrapGAppsHook4 gobject-introspection ]`;
  `buildInputs = [ gtk4 libadwaita gst_all_1.gstreamer ... ]`. Override `installPhase` (or add
  `postInstall`) to lay down data, `.desktop`, metainfo, gschema, icons, and compiled `.mo`
  translations per §9.2, then `preFixup`/`postFixup` runs `glib-compile-schemas`,
  `gtk4-update-icon-cache`, `update-desktop-database`, and `wrapGAppsHook4` wires
  `GI_TYPELIB_PATH`/`GSETTINGS_SCHEMA_DIR`/GStreamer plugin paths into the wrapper.
- **`apps.default`** = `packages.default` for `nix run`.

---

## 10. Behavior-parity checklist

Port is "done" when all of the following match the TS app:

- [ ] App IDs, resource base path, `--version`, `--help` behavior.
- [ ] Record: start/pause/resume/stop/cancel; `Recording N` naming; format + channel settings.
- [ ] Waveform: live peak drawing, drag-to-seek, high-contrast/accent recolor, alpha differences.
- [ ] Playlist: enumerate recordings dir, sort by modified desc, live dir monitor, row activation
      expand/collapse with `expanded-*` CSS.
- [ ] Playback: play/pause/seek ±10 s, position→waveform, EOS→stop, switching active row.
- [ ] Row actions: rename (with error CSS + Esc), export (FileChooserNative), delete.
- [ ] Undo toast on delete; toast dismissal trashes; undo reinserts at original index.
- [ ] GSettings profile/channel actions and their radio-toggles in the primary menu.
- [ ] Keyboard shortcuts (application.ts:111-128) including `F10`, `Ctrl+?`, `Ctrl+Q`, space, `s`,
      Delete, F2, `Ctrl+S`, `b`/`n`.
- [ ] Missing-plugin and error logging paths.
- [ ] Translations load (gettext domain = app id) and markup time strings render.

### Verification commands

```
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix develop -c cargo run          # build.rs compiles the gresource; shellHook sets the schema dir
nix build                         # flake package, wrapped with schemas/typelibs/gst plugins
```

Manual test matrix: record short/long; all four formats; mono/stereo; pause/resume/cancel;
rename; export; delete + undo; restart app to confirm enumeration/sorting; theme light/dark,
high-contrast, accent color changes; RTL locale check for waveform/time labels.

---

## 11. Phased implementation order

| Phase | Deliverable | Depends on | Exit criteria |
|---|---|---|---|
| **M0 Scaffolding** | `Cargo.toml`, `build.rs`, `config.rs`, `main.rs`, `application.rs`, window with empty/list stack, Cargo-native flake (`devShell` + `packages.default`) | — | `nix develop -c cargo run` launches; About + version work; gsettings actions resolve; `.ui` loads |
| **M1 Recorder core** | `recorder.rs`, `waveform.rs`, `recorder_widget.rs` | M0 | Can record/pause/resume/stop/cancel; live waveform + duration; format/channel honored |
| **M2 Library** | `recording.rs`, `recording_list.rs`, `recording_list_widget.rs` | M0 | Recordings enumerate, sort, monitor; rows render; peaks load/generate |
| **M3 Playback + rows** | `row.rs` (rest), player wiring | M2 | Play/pause/seek, expand/collapse, rename, export, delete |
| **M4 Window polish** | `window.rs` states, undo toasts | M1–M3 | Full state machine + undo verified |
| **M5 Packaging** | Cargo-native install: flake `packages.default` installs data/locale/schema; optional Flatpak | M4 | `nix build` produces a runnable, schema/typelib-wrapped app; optional Flatpak builds |
| **M6 Parity pass** | Bug-for-bug review + fixes | M5 | §10 checklist all green |

Rationale: M1 validates the hardest integration (GStreamer + main loop + custom cairo) early;
M2/M3 build the model/view stack on a working engine.

---

## 12. Risks and known gotchas

1. **`Gio.ListStore` subclassing** (`recording_list.rs`) is niche and verbose; budget extra time.
2. **GObject int property ranges:** current `duration` uses max `MAXINT16` for ns values
   (recorder.ts:106, recording.ts:41). Use `i64`/`u64` in Rust; do not replicate.
3. **`ValueArray` peak extraction:** API differs from GJS; verify with real `level` messages.
4. **Discoverer lifetime:** must outlive the async discovery; store it in the `Recording` imp.
5. **GStreamer bus watch on the right context:** use `add_watch_local` to avoid cross-thread
   issues; matches the GJS signal-watch behavior.
6. **Adw API baseline:** accent-color/high-contrast need libadwaita ≥ 1.6; ensure the pinned
   crate/toolkit versions expose them.
7. **`style.css` auto-load:** relies on Adw behavior; verify and fall back to a `CssProvider`.
8. **Bug-for-bug fidelity decisions** to make explicitly: `seek-backward` empty handler
   (row.ts:213-217), `current_peak` clamp oddity (recorder.ts:322), `formatTime` 24h wrap
   (utils.ts:25), waveform string-peaks (waveform.ts:206). Document each choice in the commit.
9. **Action groups and accelerators:** `recording.*` groups are per-row widgets; `app.*` accels
   resolve via focus. Reproduce insertion points exactly.
10. **i18n init:** GJS's `pkg.initGettext()`/`initFormat()` must be replaced with explicit
    `gettextrs::setlocale`/`textdomain`/`bindtextdomain(LOCALEDIR, APP_ID)` in `main`.
11. **Profile suffix in resources:** JS resource path used `@profile@`, but data resources and
    templates do **not**; ensure `RESOURCE_BASE_PATH` stays constant across profiles.
12. **Flatpak Rust vendoring** is the main packaging risk; decide network vs vendored early in M5.

---

## 13. Feature work note (out of scope here)

The planned trim-range feature (single start/end per recording, sidecar JSON next to the audio in
`RecordingsDir`, playback/navigation bounds only) is **deferred** until after the port's M6. Its
design must account for `RecordingList` indexing sidecar files and for rename/delete/export sync.
Porting first with faithful behavior makes that feature a localized addition to `recording.rs`,
`waveform.rs`, and `row.rs`.

---

## 14. Appendix — Lean port roadmap (later, incremental)

These are **not** part of the faithful port. Apply opportunistically after parity, one at a time,
verifying behavior each step. They reduce LOC and boilerplate by avoiding GObject modeling where
it isn't required. Estimated savings are relative to the faithful port.

### 14.1 Use `serde`/`serde_json` everywhere
- **Target:** peaks cache in `recording.rs`; future trim metadata.
- **Change:** define `#[derive(Serialize, Deserialize)] struct Peaks(Vec<f32>)`; replace
  hand-rolled JSON. The faithful port already uses `serde_json`, so this is mostly tightening types.
- **Saving:** ~30–80 lines. **Risk:** low.

### 14.2 Replace `Gio.ListStore` subclass with `BoxedAnyObject`
- **Target:** `recording_list.rs`, `recording_list_widget.rs`.
- **Change:** store `glib::BoxedAnyObject::new(RecordingData)` (a plain struct) in a plain
  `gio::ListStore::new::<glib::BoxedAnyObject>()`; keep `bind_model`.
- **Saving:** ~150–250 lines and removes the hardest subclass. **Risk:** medium — model item type
  changes; monitor/sort logic stays.

### 14.3 Drop GObject subclassing for non-widget models
- **Target:** `recorder.rs` (`Recorder`), `recording.rs` (`Recording`).
- **Change:** plain structs with `Rc<RefCell<..>>`/channels; replace signals with callbacks or a
  small observer; drive widgets manually instead of `bind_property`.
- **Saving:** ~150–300 lines. **Risk:** medium–high — loses property binding ergonomics; do only
  after the list is stable.

### 14.4 Avoid widget subclassing where templates aren't required
- **Target:** `window.rs`, `row.rs`, `recorder_widget.rs`.
- **Change:** `gtk::Builder` + `#[template_child]` without `glib::wrapper!`; hold state in `Rc`.
- **Saving:** ~20–35% of those files. **Risk:** medium — loses GObject signals; migrate one widget
  at a time.

### 14.5 Use modern `glib::clone!` `#[weak]`/`#[strong]` closure attributes
- **Target:** all widget files.
- **Change:** replace manual weak-ref plumbing with `#[weak]`/`#[upgrade_or_default]` closure
  setup.
- **Saving:** 50–150 lines across the codebase. **Risk:** low.

### 14.6 Do **not** introduce `tokio`
- GTK/Gio/Gst are main-thread-bound; a second executor requires cross-thread marshalling and
  `Send` juggling. Keep `glib` futures on the `MainContext`.
- **Impact if done anyway:** +200–400 lines and new race-condition classes.

### 14.7 Do **not** replace GStreamer with Rust-native DSP
- Would require reimplementing capture (`cpal`), decode (`symphonia`/`hound`), and per-codec
  encoders (vorbis/opus/flac/mp3/m4a), losing device selection, `encodebin`, `GstPlayer`, and
  `Discoverer`. **Impact:** +1,500–3,000 lines, months of work, feature regressions.

### 14.8 Optional: migrate the list to `Gtk.ListView` + `SignalListItemFactory`
- Modern replacement for `Gtk.ListBox::bind_model`; pairs well with 14.2.
- **Saving:** neutral-to-positive LOC; better scalability. **Risk:** medium — behavior-parity
  re-verification required for expand/collapse CSS and row activation.

Recommended sequencing of lean improvements: **14.1 → 14.5 → 14.2 → 14.4 → 14.3 → 14.8**.
Each step is independently reversible.
