mod application;
mod config;
mod window;

use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    unsafe {
        let _ = gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    }
    if let Some(dir) = config::locale_dir() {
        let _ = gettextrs::bindtextdomain(config::GETTEXT_PACKAGE, dir);
    }
    let _ = gettextrs::textdomain(config::GETTEXT_PACKAGE);

    gio::resources_register_include!("app.drey.Vocalis.data.gresource")
        .expect("Failed to register resources");

    glib::set_application_name("Vocalis");

    let app = application::Application::new();
    app.run()
}
