use gtk::prelude::*;
use gtk::{gio, glib};

pub const APP_ID: &str = if cfg!(feature = "development") {
    "app.drey.Vocalis.Devel"
} else {
    "app.drey.Vocalis"
};

pub const RESOURCE_BASE_PATH: &str = "/app/drey/Vocalis/";
pub const GETTEXT_PACKAGE: &str = APP_ID;

/// `CARGO_PKG_VERSION`, plus the optional development suffix injected by the
/// Meson/Flatpak build (`VOCALIS_VERSION_SUFFIX`).
pub fn version() -> String {
    let base = env!("CARGO_PKG_VERSION");
    match option_env!("VOCALIS_VERSION_SUFFIX") {
        Some(suffix) => format!("{base}{suffix}"),
        None => base.to_string(),
    }
}

pub fn recordings_dir() -> gio::File {
    gio::File::for_path(glib::user_data_dir().join(APP_ID))
}

pub fn cache_dir() -> gio::File {
    gio::File::for_path(glib::user_cache_dir().join(APP_ID))
}

#[allow(dead_code)]
pub fn settings() -> gio::Settings {
    gio::Settings::new(APP_ID)
}

pub fn locale_dir() -> Option<String> {
    std::env::var("VOCALIS_LOCALEDIR")
        .ok()
        .or_else(|| option_env!("VOCALIS_LOCALEDIR").map(str::to_string))
}

pub fn init_user_directory(dir: &gio::File) {
    if let Err(err) = dir.make_directory_with_parents(gio::Cancellable::NONE) {
        if !err.matches(gio::IOErrorEnum::Exists) {
            eprintln!("Failed to create directory: {err}");
        }
    }
}
