use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::recorder::Recorder;
use crate::recorder_widget::RecorderWidget;
use crate::recording::Recording;
use crate::recording_list::RecordingList;
use crate::recording_list_widget::RecordingsListWidget;
use crate::row::Row;

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

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/app/drey/Vocalis/ui/window.ui")]
    pub struct Window {
        #[template_child(id = "mainStack")]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child(id = "emptyPage")]
        pub empty_page: TemplateChild<adw::StatusPage>,
        #[template_child(id = "recordingsList")]
        pub recordings_list: TemplateChild<RecordingsListWidget>,
        #[template_child(id = "recorderWidget")]
        pub recorder_widget: TemplateChild<RecorderWidget>,
        #[template_child(id = "toastOverlay")]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child(id = "toolbarView")]
        pub toolbar_view: TemplateChild<adw::ToolbarView>,

        state: Cell<WindowState>,
        recorder: RefCell<Option<Recorder>>,
        player: RefCell<Option<gst_player::Player>>,
        recording_list: RefCell<Option<RecordingList>>,
        items_signal_id: RefCell<Option<glib::SignalHandlerId>>,
        toast_undo: Cell<bool>,
        undo_toasts: RefCell<Vec<adw::Toast>>,
        undo_signal_id: RefCell<Option<glib::SignalHandlerId>>,
        undo_action: RefCell<Option<gio::SimpleAction>>,
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

    impl WindowImpl for Window {
        fn close_request(&self) -> glib::Propagation {
            // Clone first: `dismiss()` emits `dismissed` synchronously, and the
            // handler mutates `undo_toasts`, so we must not hold a borrow here.
            let toasts: Vec<adw::Toast> = self.undo_toasts.borrow().clone();
            for toast in toasts {
                toast.dismiss();
            }
            self.undo_toasts.borrow_mut().clear();

            if let Some(list) = self.recording_list.borrow().as_ref() {
                list.cancellable().cancel();

                if let Some(id) = self.items_signal_id.borrow_mut().take() {
                    list.store().disconnect(id);
                }

                let store = list.store();
                for i in 0..store.n_items() {
                    if let Some(item) = store.item(i) {
                        if let Ok(recording) = item.downcast::<Recording>() {
                            recording.stop_pipeline();
                        }
                    }
                }
            }

            if let Some(recorder) = self.recorder.borrow().as_ref() {
                if recorder.is_active() {
                    recorder.stop();
                }
            }

            glib::Propagation::Proceed
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}

    impl Window {
        pub fn setup_recorder(&self) {
            let recorder = Recorder::new();
            let widget = self.recorder_widget.get();

            widget.setup(&recorder);
            self.obj()
                .insert_action_group("recorder", Some(&widget.actions_group()));

            widget.connect_started(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |_| {
                    window.imp().on_recorder_started();
                }
            ));

            widget.connect_canceled(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |_| {
                    window.imp().on_recorder_canceled();
                }
            ));

            widget.connect_stopped(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |_, recording| {
                    window.imp().on_recorder_stopped(recording);
                }
            ));

            *self.recorder.borrow_mut() = Some(recorder);
        }

        pub fn setup_library(&self) {
            let dispatcher = gst_player::PlayerGMainContextSignalDispatcher::new(None);
            let player =
                gst_player::Player::new(None::<gst_player::PlayerVideoRenderer>, Some(dispatcher));
            player.connect_end_of_stream(|player| player.stop());

            let recording_list = RecordingList::new();
            let list_widget = self.recordings_list.get();
            list_widget.setup(recording_list.store(), &player);

            list_widget.connect_row_deleted(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |_widget, recording, index| {
                    let imp = window.imp();
                    if let Some(list) = imp.recording_list.borrow().as_ref() {
                        list.store().remove(index);
                    }

                    let name = recording.name();
                    let message = if !name.is_empty() {
                        gettextrs::gettext("\"%s\" deleted").replace("%s", &name)
                    } else {
                        gettextrs::gettext("Recording deleted")
                    };
                    imp.send_notification(&message, recording, index);
                }
            ));

            let items_signal_id = recording_list.store().connect_items_changed(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |store, _position, _removed, _added| {
                    let imp = window.imp();
                    if imp.state.get() != WindowState::Recorder {
                        if store.n_items() == 0 {
                            imp.set_state(WindowState::Empty);
                        } else {
                            imp.set_state(WindowState::List);
                        }
                    }
                }
            ));

            let undo_action = gio::SimpleAction::new("undo", None);
            self.obj().add_action(&undo_action);

            let open_menu_action =
                gio::SimpleAction::new_stateful("open-primary-menu", None, &true.to_variant());
            open_menu_action.connect_activate(|action, _| {
                let state = action
                    .state()
                    .and_then(|value| value.get::<bool>())
                    .unwrap_or(false);
                action.set_state(&(!state).to_variant());
            });
            self.obj().add_action(&open_menu_action);

            // Context-dependent transport keys (see Application::init_actions).
            let play_pause_action = gio::SimpleAction::new("play-pause", None);
            play_pause_action.connect_activate(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |_, _| {
                    window.imp().on_play_pause();
                }
            ));
            self.obj().add_action(&play_pause_action);

            let delete_recording_action = gio::SimpleAction::new("delete-recording", None);
            delete_recording_action.connect_activate(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |_, _| {
                    window.imp().on_delete_recording();
                }
            ));
            self.obj().add_action(&delete_recording_action);

            *self.player.borrow_mut() = Some(player);
            *self.recording_list.borrow_mut() = Some(recording_list);
            *self.items_signal_id.borrow_mut() = Some(items_signal_id);
            *self.undo_action.borrow_mut() = Some(undo_action);
        }

        fn on_play_pause(&self) {
            match self.state.get() {
                WindowState::Recorder => {
                    let group = self.recorder_widget.get().actions_group();
                    for name in ["pause", "resume"] {
                        if let Some(action) = group.lookup_action(name) {
                            if action.is_enabled() {
                                action.activate(None);
                                return;
                            }
                        }
                    }
                }
                WindowState::List => {
                    self.recordings_list.get().toggle_playback();
                }
                WindowState::Empty => {}
            }
        }

        fn on_delete_recording(&self) {
            match self.state.get() {
                WindowState::Recorder => {
                    let widget = self.recorder_widget.get();
                    if let Some(action) = widget.actions_group().lookup_action("cancel") {
                        if action.is_enabled() {
                            action.activate(None);
                        }
                    }
                }
                WindowState::List => {
                    self.recordings_list.get().delete_active();
                }
                WindowState::Empty => {}
            }
        }

        fn on_recorder_started(&self) {
            if let Some(player) = self.player.borrow().as_ref() {
                player.stop();
            }
            self.recordings_list.get().stop_editing();
            self.set_state(WindowState::Recorder);
        }

        fn on_recorder_canceled(&self) {
            let empty = self
                .recording_list
                .borrow()
                .as_ref()
                .is_none_or(|list| list.store().n_items() == 0);
            self.set_state(if empty {
                WindowState::Empty
            } else {
                WindowState::List
            });
        }

        fn on_recorder_stopped(&self, recording: Option<Recording>) {
            if let Some(recording) = recording {
                let store = self
                    .recording_list
                    .borrow()
                    .as_ref()
                    .map(|list| list.store().clone());
                let list = self.recordings_list.get().list();

                if let Some(store) = store {
                    store.insert(0, &recording);
                }
                if let Some(list) = list {
                    if let Some(row) = list
                        .row_at_index(0)
                        .and_then(|row| row.downcast::<Row>().ok())
                    {
                        row.set_edit_mode(true);
                    }
                }
            }

            self.set_state(WindowState::List);
        }

        fn send_notification(&self, message: &str, recording: &Recording, index: u32) {
            let toast = adw::Toast::new(message);
            let title = toast.title();

            let dismiss_recording = recording.clone();
            toast.connect_dismissed(glib::clone!(
                #[weak(rename_to = window)]
                self.obj(),
                move |_toast| {
                    let imp = window.imp();
                    if !imp.toast_undo.get() {
                        dismiss_recording.delete();
                    }
                    imp.toast_undo.set(false);
                    imp.undo_toasts
                        .borrow_mut()
                        .retain(|toast| toast.title() != title);
                }
            ));

            if let Some(undo_action) = self.undo_action.borrow().as_ref() {
                if let Some(id) = self.undo_signal_id.borrow_mut().take() {
                    undo_action.disconnect(id);
                }

                let undo_recording = recording.clone();
                let id = undo_action.connect_activate(glib::clone!(
                    #[weak(rename_to = window)]
                    self.obj(),
                    move |_, _| {
                        let imp = window.imp();
                        if let Some(list) = imp.recording_list.borrow().as_ref() {
                            list.store().insert(index, &undo_recording);
                        }
                        imp.toast_undo.set(true);
                    }
                ));
                *self.undo_signal_id.borrow_mut() = Some(id);
            }

            toast.set_action_name(Some("win.undo"));
            toast.set_button_label(Some(&gettextrs::gettext("Undo")));
            self.toast_overlay.add_toast(toast.clone());
            self.undo_toasts.borrow_mut().push(toast);
        }

        pub(crate) fn set_state(&self, state: WindowState) {
            let (visible_child, reveal_top_bars) = match state {
                WindowState::Recorder => ("recorder", false),
                WindowState::List => ("recordings", true),
                WindowState::Empty => ("empty", true),
            };

            self.main_stack.set_visible_child_name(visible_child);
            self.toolbar_view.set_reveal_top_bars(reveal_top_bars);

            // Only the recordings list owns `recording.*` shortcuts; while the
            // recorder is shown we let the `recorder.*` accelerators take over.
            self.recordings_list
                .get()
                .set_shortcuts_active(state == WindowState::List);

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
        // Register the leaf GTypes before the window template is loaded so that
        // GtkBuilder can resolve `$VocalisRecorderWidget` / `$VocalisRecordingsListWidget`.
        RecorderWidget::static_type();
        RecordingsListWidget::static_type();

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
        window.imp().setup_library();
        window.imp().set_state(WindowState::Empty);

        window
    }
}
