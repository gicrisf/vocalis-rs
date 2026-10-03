use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::Object;
use gtk::{gio, glib};

use crate::config;
use crate::window::Window;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Application;

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "VocalisApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn startup(&self) {
            self.parent_startup();

            gst::init().expect("Failed to initialize GStreamer");

            let app = self.obj();
            config::init_user_directory(&config::recordings_dir());
            config::init_user_directory(&config::cache_dir());
            app.init_actions();
        }

        fn activate(&self) {
            let app = self.obj().clone();
            let window = app
                .active_window()
                .unwrap_or_else(|| Window::new(&app).upcast());
            window.present();
        }
    }

    impl GtkApplicationImpl for Application {}

    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Application {
    pub fn new() -> Self {
        Object::builder()
            .property("application-id", config::APP_ID)
            .property("resource-base-path", config::RESOURCE_BASE_PATH)
            .build()
    }

    fn init_actions(&self) {
        let about_action = gio::SimpleAction::new("about", None);
        let app = self.clone();
        about_action.connect_activate(move |_, _| app.show_about());
        self.add_action(&about_action);

        let quit_action = gio::SimpleAction::new("quit", None);
        let app = self.clone();
        quit_action.connect_activate(move |_, _| {
            if let Some(window) = app.active_window() {
                window.close();
            }
        });
        self.add_action(&quit_action);

        self.set_accels_for_action("app.quit", &["<Primary>q"]);
        self.set_accels_for_action("win.show-help-overlay", &["<Primary>question"]);
    }

    fn show_about(&self) {
        let application_name = glib::application_name().unwrap_or_else(|| "Vocalis".into());
        let translator_credits = gettextrs::gettext("translator-credits");
        let developers: &[&str] = &[
            "Christopher Davis <christopherdavis@gnome.org>",
            "Meg Ford <megford@gnome.org>",
            "Bilal Elmoussaoui <bil.elmoussaoui@gmail.com>",
            "Felipe Borges <felipeborges@gnome.org>",
            "Kavan Mevada <kavanmevada@gmail.com>",
        ];
        let artists: &[&str] = &[
            "Reda Lazri <the.red.shortcut@gmail.com>",
            "Garrett LeSage <garrettl@gmail.com>",
            "Hylke Bons <hylkebons@gmail.com>",
            "Sam Hewitt <hewittsamuel@gmail.com>",
        ];

        let about = adw::AboutDialog::builder()
            .application_name(application_name.as_str())
            .application_icon(config::APP_ID)
            .version(config::VERSION)
            .license_type(gtk::License::Gpl20)
            .website("https://gitlab.gnome.org/World/vocalis")
            .issue_url("https://gitlab.gnome.org/World/vocalis/-/issues")
            .copyright(
                "Copyright 2013-2019 Meg Ford\n\
                 Copyright 2019-2020 Bilal Elmoussaoui & Felipe Borges\n\
                 Copyright 2024 Christopher Davis",
            )
            .developers(developers)
            .artists(artists)
            .translator_credits(translator_credits.as_str())
            .build();

        about.present(self.active_window().as_ref());
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
