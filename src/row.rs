use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::recording::Recording;
use crate::utils::{display_date_time, format_time};
use crate::waveform::{WaveForm, WaveType};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Playing,
    #[default]
    Paused,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::Row)]
    #[template(resource = "/app/drey/Vocalis/ui/row.ui")]
    pub struct Row {
        #[template_child(id = "playbackStack")]
        pub playback_stack: TemplateChild<gtk::Stack>,
        #[template_child(id = "mainStack")]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child(id = "waveformStack")]
        pub waveform_stack: TemplateChild<gtk::Stack>,
        #[template_child(id = "rightStack")]
        pub right_stack: TemplateChild<gtk::Stack>,
        #[template_child(id = "name")]
        pub name: TemplateChild<gtk::Label>,
        #[template_child(id = "entry")]
        pub entry: TemplateChild<gtk::Entry>,
        #[template_child(id = "date")]
        pub date: TemplateChild<gtk::Label>,
        #[template_child(id = "duration")]
        pub duration: TemplateChild<gtk::Label>,
        #[template_child(id = "revealer")]
        pub revealer: TemplateChild<gtk::Revealer>,
        #[template_child(id = "playbackControls")]
        pub playback_controls: TemplateChild<gtk::Box>,
        #[template_child(id = "saveBtn")]
        pub save_btn: TemplateChild<gtk::Button>,
        #[template_child(id = "playBtn")]
        pub play_btn: TemplateChild<gtk::Button>,
        #[template_child(id = "pauseBtn")]
        pub pause_btn: TemplateChild<gtk::Button>,

        #[property(get, set = Self::set_expanded)]
        pub expanded: Cell<bool>,
        pub recording: RefCell<Option<Recording>>,
        pub waveform: RefCell<Option<WaveForm>>,
        pub edit_mode: Cell<bool>,
        pub state: Cell<RowState>,
        pub action_group: RefCell<Option<gio::SimpleActionGroup>>,
        pub save_rename_action: RefCell<Option<gio::SimpleAction>>,
        pub rename_action: RefCell<Option<gio::SimpleAction>>,
        pub pause_action: RefCell<Option<gio::SimpleAction>>,
        pub play_action: RefCell<Option<gio::SimpleAction>>,
        pub key_controller: RefCell<Option<gtk::EventControllerKey>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "VocalisRow";
        type Type = super::Row;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Row {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("play")
                        .param_types([glib::Type::STRING])
                        .build(),
                    glib::subclass::Signal::builder("pause").build(),
                    glib::subclass::Signal::builder("seek-backward").build(),
                    glib::subclass::Signal::builder("seek-forward").build(),
                    glib::subclass::Signal::builder("deleted").build(),
                ]
            })
        }
    }

    impl WidgetImpl for Row {}
    impl ListBoxRowImpl for Row {}

    impl Row {
        pub fn setup(&self, recording: &Recording) {
            *self.recording.borrow_mut() = Some(recording.clone());

            let waveform = WaveForm::new(WaveType::Player);
            waveform.set_margin_top(18);
            waveform.set_height_request(60);
            self.waveform_stack.add_named(&waveform, Some("wave"));

            if !recording.peaks().is_empty() {
                waveform.set_peaks(recording.peaks());
                self.waveform_stack.set_visible_child_name("wave");
            } else {
                recording.load_peaks();
            }

            self.date
                .set_label(&display_date_time(&recording.time_modified()));

            recording
                .bind_property("name", &*self.name, "label")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::DEFAULT)
                .build();
            recording
                .bind_property("name", &*self.entry, "text")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::DEFAULT)
                .build();
            self.obj()
                .bind_property("expanded", &*self.revealer, "reveal-child")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::DEFAULT)
                .build();

            *self.waveform.borrow_mut() = Some(waveform);

            self.setup_actions(recording);

            let waveform = self.waveform.borrow().clone().unwrap();
            waveform.connect_local(
                "gesture-pressed",
                false,
                glib::clone!(
                    #[weak(rename_to = row)]
                    self.obj(),
                    #[upgrade_or]
                    None,
                    move |_| {
                        if let Some(action) = row.imp().pause_action.borrow().as_ref() {
                            action.activate(None);
                        }
                        None
                    }
                ),
            );

            let controller = gtk::EventControllerKey::new();
            controller.connect_key_pressed(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_controller, key, _code, _state| {
                    let imp = row.imp();
                    imp.entry.remove_css_class("error");
                    if key == gdk::Key::Escape {
                        imp.set_edit_mode(false);
                    }
                    glib::Propagation::Proceed
                }
            ));
            self.entry.add_controller(controller.clone());
            *self.key_controller.borrow_mut() = Some(controller);

            self.entry.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_| {
                    if let Some(action) = row.imp().save_rename_action.borrow().as_ref() {
                        action.activate(None);
                    }
                }
            ));

            recording.connect_peaks_updated(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |recording| {
                    row.imp().waveform_stack.set_visible_child_name("wave");
                    if let Some(waveform) = row.imp().waveform.borrow().as_ref() {
                        waveform.set_peaks(recording.peaks());
                    }
                }
            ));

            recording.connect_peaks_loading(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_recording| {
                    row.imp().waveform_stack.set_visible_child_name("loading");
                }
            ));

            self.playback_controls
                .set_direction(gtk::TextDirection::Ltr);
            self.duration.set_direction(gtk::TextDirection::Ltr);
            self.duration.set_markup(&format_time(recording.duration()));

            let duration_label = self.duration.clone();
            recording.connect_notify_local(Some("duration"), move |recording, _| {
                duration_label.set_markup(&format_time(recording.duration()));
            });
        }

        fn setup_actions(&self, recording: &Recording) {
            let group = gio::SimpleActionGroup::new();

            let export_action = gio::SimpleAction::new("export", None);
            export_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    row.imp().on_export();
                }
            ));
            group.add_action(&export_action);

            let save_rename_action = gio::SimpleAction::new("save", None);
            save_rename_action.set_enabled(false);
            save_rename_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    row.imp().on_rename();
                }
            ));
            group.add_action(&save_rename_action);

            let rename_action = gio::SimpleAction::new("rename", None);
            rename_action.set_enabled(true);
            rename_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    let imp = row.imp();
                    imp.set_edit_mode(true);
                    if let Some(action) = imp.rename_action.borrow().as_ref() {
                        action.set_enabled(false);
                    }
                }
            ));
            rename_action
                .bind_property("enabled", &save_rename_action, "enabled")
                .flags(glib::BindingFlags::INVERT_BOOLEAN)
                .build();
            group.add_action(&rename_action);

            let pause_action = gio::SimpleAction::new("pause", None);
            pause_action.set_enabled(false);
            pause_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    row.emit_by_name::<()>("pause", &[]);
                    row.imp().set_state(RowState::Paused);
                }
            ));
            group.add_action(&pause_action);

            let play_action = gio::SimpleAction::new("play", None);
            play_action.set_enabled(true);
            let uri = recording.uri();
            play_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    row.emit_by_name::<()>("play", &[&uri]);
                    row.imp().set_state(RowState::Playing);
                }
            ));
            group.add_action(&play_action);

            let delete_action = gio::SimpleAction::new("delete", None);
            delete_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    row.emit_by_name::<()>("deleted", &[]);
                }
            ));
            group.add_action(&delete_action);

            let seek_back_action = gio::SimpleAction::new("seek-backward", None);
            seek_back_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    row.emit_by_name::<()>("seek-backward", &[]);
                }
            ));
            group.add_action(&seek_back_action);

            let seek_forward_action = gio::SimpleAction::new("seek-forward", None);
            seek_forward_action.connect_activate(glib::clone!(
                #[weak(rename_to = row)]
                self.obj(),
                move |_, _| {
                    row.emit_by_name::<()>("seek-forward", &[]);
                }
            ));
            group.add_action(&seek_forward_action);

            // The "recording" group is inserted on the toplevel window by
            // RecordingsListWidget for the active row (see `sync_window_actions`),
            // so that application accelerators can reach these actions regardless
            // of focus. The row's buttons resolve them through the window.

            *self.save_rename_action.borrow_mut() = Some(save_rename_action);
            *self.rename_action.borrow_mut() = Some(rename_action);
            *self.pause_action.borrow_mut() = Some(pause_action);
            *self.play_action.borrow_mut() = Some(play_action);
            *self.action_group.borrow_mut() = Some(group);
        }

        fn on_export(&self) {
            let recording = self.recording.borrow().clone().unwrap();
            let parent = self
                .obj()
                .root()
                .and_then(|root| root.downcast::<gtk::Window>().ok());

            let dialog = gtk::FileDialog::new();
            dialog.set_title(&gettextrs::gettext("Export Recording"));
            dialog.set_accept_label(Some(&gettextrs::gettext("_Export")));
            dialog.set_initial_name(Some(&format!(
                "{}.{}",
                recording.name(),
                recording.extension().unwrap_or_default()
            )));

            glib::MainContext::default().spawn_local(async move {
                match dialog.save_future(parent.as_ref()).await {
                    Ok(dest) => recording.save(&dest),
                    Err(err)
                        if err.matches(gtk::DialogError::Dismissed)
                            || err.matches(gtk::DialogError::Cancelled) => {}
                    Err(err) => eprintln!("Export failed: {err}"),
                }
            });
        }

        fn on_rename(&self) {
            let entry_text = self.entry.text().to_string();
            let name_label = self.name.label().to_string();
            let recording = self.recording.borrow().clone().unwrap();

            let success = if name_label != entry_text {
                recording.rename(&entry_text).is_ok()
            } else {
                true
            };

            if success {
                self.set_edit_mode(false);
                if let Some(action) = self.rename_action.borrow().as_ref() {
                    action.set_enabled(true);
                }
                self.entry.remove_css_class("error");
            } else {
                self.entry.add_css_class("error");
            }
        }

        pub fn set_expanded(&self, state: bool) {
            self.expanded.set(state);
            self.obj().notify("expanded");
        }

        pub fn set_edit_mode(&self, state: bool) {
            self.main_stack
                .set_visible_child_name(if state { "edit" } else { "display" });
            self.edit_mode.set(state);

            if state {
                if !self.expanded.get() {
                    self.obj().activate();
                }
                self.entry.grab_focus();
                self.right_stack.set_visible_child_name("save");
            } else {
                self.right_stack.set_visible_child_name("options");
                self.obj().grab_focus();
            }

            if let Some(group) = self.action_group.borrow().as_ref() {
                for name in group.list_actions() {
                    if name.as_str() != "save" {
                        if let Some(action) = group.lookup_action(name.as_str()) {
                            if let Ok(action) = action.downcast::<gio::SimpleAction>() {
                                action.set_enabled(!state);
                            }
                        }
                    }
                }
            }
        }

        pub fn set_state(&self, row_state: RowState) {
            self.state.set(row_state);

            let play = self.play_action.borrow().clone();
            let pause = self.pause_action.borrow().clone();

            match row_state {
                RowState::Playing => {
                    if let Some(action) = play {
                        action.set_enabled(false);
                    }
                    if let Some(action) = pause {
                        action.set_enabled(true);
                    }
                    self.playback_stack.set_visible_child_name("pause");
                    self.pause_btn.grab_focus();
                }
                RowState::Paused => {
                    if let Some(action) = play {
                        action.set_enabled(true);
                    }
                    if let Some(action) = pause {
                        action.set_enabled(false);
                    }
                    self.playback_stack.set_visible_child_name("play");
                    self.play_btn.grab_focus();
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct Row(ObjectSubclass<imp::Row>)
        @extends gtk::Widget, gtk::ListBoxRow,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl Row {
    pub fn new(recording: &Recording) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().setup(recording);
        obj
    }

    pub fn recording(&self) -> Recording {
        self.imp().recording.borrow().clone().unwrap()
    }

    pub fn action_group(&self) -> Option<gio::SimpleActionGroup> {
        self.imp().action_group.borrow().clone()
    }

    pub fn waveform(&self) -> WaveForm {
        self.imp().waveform.borrow().clone().unwrap()
    }

    pub fn edit_mode(&self) -> bool {
        self.imp().edit_mode.get()
    }

    pub fn set_edit_mode(&self, state: bool) {
        self.imp().set_edit_mode(state);
    }

    pub fn set_state(&self, state: RowState) {
        self.imp().set_state(state);
    }

    pub fn connect_play<F: Fn(&Self, &str) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("play", false, move |values| {
            let obj = values[0].get::<Row>().unwrap();
            let uri = values[1].get::<String>().unwrap();
            f(&obj, &uri);
            None
        })
    }

    pub fn connect_pause<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("pause", false, move |values| {
            let obj = values[0].get::<Row>().unwrap();
            f(&obj);
            None
        })
    }

    pub fn connect_seek_backward<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("seek-backward", false, move |values| {
            let obj = values[0].get::<Row>().unwrap();
            f(&obj);
            None
        })
    }

    pub fn connect_seek_forward<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("seek-forward", false, move |values| {
            let obj = values[0].get::<Row>().unwrap();
            f(&obj);
            None
        })
    }

    pub fn connect_deleted<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("deleted", false, move |values| {
            let obj = values[0].get::<Row>().unwrap();
            f(&obj);
            None
        })
    }
}
