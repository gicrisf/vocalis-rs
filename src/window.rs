use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/app/drey/Vocalis/ui/window.ui")]
    pub struct Window {
        #[template_child(id = "mainStack")]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child(id = "emptyPage")]
        pub empty_page: TemplateChild<adw::StatusPage>,
        #[template_child(id = "column")]
        pub column: TemplateChild<adw::Clamp>,
        #[template_child(id = "toastOverlay")]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child(id = "toolbarView")]
        pub toolbar_view: TemplateChild<adw::ToolbarView>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "VocalisWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {}
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &impl IsA<adw::Application>) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();

        window.set_icon_name(Some(config::APP_ID));
        window
            .imp()
            .empty_page
            .set_icon_name(Some(&format!("{}-symbolic", config::APP_ID)));

        if config::APP_ID.ends_with("Devel") {
            window.add_css_class("devel");
        }

        window
    }
}
