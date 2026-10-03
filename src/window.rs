use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::recorder::Recorder;
use crate::recorder_widget::RecorderWidget;

#[allow(dead_code)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowState {
    #[default]
    Empty,
    List,
    Recorder,
}

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

        state: Cell<WindowState>,
        recorder: RefCell<Option<Recorder>>,
        recorder_widget: RefCell<Option<RecorderWidget>>,
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

    impl Window {
        pub fn setup_recorder(&self) {
            let recorder = Recorder::new();
            let widget = RecorderWidget::new(&recorder);

            self.main_stack.add_named(&widget, Some("recorder"));
            self.obj()
                .insert_action_group("recorder", Some(&widget.actions_group()));

            let window = self.obj().downgrade();
            widget.connect_started(move |_| {
                if let Some(window) = window.upgrade() {
                    window.imp().on_recorder_started();
                }
            });

            let window = self.obj().downgrade();
            widget.connect_canceled(move |_| {
                if let Some(window) = window.upgrade() {
                    window.imp().on_recorder_canceled();
                }
            });

            let window = self.obj().downgrade();
            widget.connect_stopped(move |_| {
                if let Some(window) = window.upgrade() {
                    window.imp().on_recorder_stopped();
                }
            });

            *self.recorder.borrow_mut() = Some(recorder);
            *self.recorder_widget.borrow_mut() = Some(widget);
        }

        fn on_recorder_started(&self) {
            self.set_state(WindowState::Recorder);
        }

        fn on_recorder_canceled(&self) {
            self.set_state(WindowState::Empty);
        }

        fn on_recorder_stopped(&self) {
            self.set_state(WindowState::Empty);
        }

        pub(crate) fn set_state(&self, state: WindowState) {
            let (visible_child, reveal_top_bars) = match state {
                WindowState::Recorder => ("recorder", false),
                WindowState::List => ("recordings", true),
                WindowState::Empty => ("empty", true),
            };

            self.main_stack.set_visible_child_name(visible_child);
            self.toolbar_view.set_reveal_top_bars(reveal_top_bars);
            self.state.set(state);
        }
    }
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

        window.imp().setup_recorder();
        window.imp().set_state(WindowState::Empty);

        window
    }
}
