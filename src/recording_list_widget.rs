use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::recording::Recording;
use crate::row::{Row, RowState};

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/app/drey/Vocalis/ui/recordings-list.ui")]
    pub struct RecordingsListWidget {
        player: RefCell<Option<gst_player::Player>>,
        #[template_child(id = "list")]
        pub list: TemplateChild<gtk::ListBox>,
        pub active_row: RefCell<Option<Row>>,
        active_playing_row: RefCell<Option<Row>>,
        shortcuts_active: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecordingsListWidget {
        const NAME: &'static str = "VocalisRecordingsListWidget";
        type Type = super::RecordingsListWidget;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RecordingsListWidget {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![glib::subclass::Signal::builder("row-deleted")
                    .param_types([Recording::static_type(), glib::Type::U32])
                    .build()]
            })
        }
    }

    impl WidgetImpl for RecordingsListWidget {}
    impl BinImpl for RecordingsListWidget {}

    impl RecordingsListWidget {
        pub fn setup(&self, model: &gio::ListStore, player: &gst_player::Player) {
            let list = self.list.get();

            *self.player.borrow_mut() = Some(player.clone());

            let widget = glib::SendWeakRef::from(self.obj().downgrade());
            player.connect_state_changed(move |_player, state| {
                if let Some(widget) = widget.upgrade() {
                    widget.imp().on_state_changed(state);
                }
            });

            let widget = glib::SendWeakRef::from(self.obj().downgrade());
            player.connect_position_updated(move |_player, position| {
                if let Some(widget) = widget.upgrade() {
                    widget.imp().on_position_updated(position);
                }
            });

            list.bind_model(
                Some(model),
                glib::clone!(
                    #[weak_allow_none(rename_to = widget)]
                    self.obj(),
                    move |item| {
                        let recording = item
                            .clone()
                            .downcast::<Recording>()
                            .expect("model item is not a Recording");
                        let row = Row::new(&recording);

                        if let Some(widget) = widget.as_ref() {
                            widget.imp().wire_row(&row, &recording);
                        }

                        row.upcast()
                    }
                ),
            );

            list.connect_row_activated(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |_list, row| {
                    if let Ok(row) = row.clone().downcast::<Row>() {
                        widget.imp().on_row_activated(&row);
                    }
                }
            ));
        }

        fn wire_row(&self, row: &Row, recording: &Recording) {
            let uri = recording.uri();
            let row_obj = row.clone();
            row.waveform().connect_local(
                "gesture-pressed",
                false,
                glib::clone!(
                    #[weak(rename_to = widget)]
                    self.obj(),
                    #[weak]
                    row_obj,
                    #[upgrade_or]
                    None,
                    move |_| {
                        let imp = widget.imp();
                        let is_same = imp
                            .active_playing_row
                            .borrow()
                            .as_ref()
                            .is_some_and(|active| active.as_ptr() == row_obj.as_ptr());
                        if !is_same {
                            if let Some(active) = imp.active_playing_row.borrow().as_ref() {
                                active.waveform().set_position_value(0.0);
                            }
                            *imp.active_playing_row.borrow_mut() = Some(row_obj.clone());
                            if let Some(player) = imp.player.borrow().as_ref() {
                                player.set_uri(Some(&uri));
                            }
                        }
                        None
                    }
                ),
            );

            let row_obj = row.clone();
            row.waveform().connect_local(
                "position-changed",
                false,
                glib::clone!(
                    #[weak(rename_to = widget)]
                    self.obj(),
                    #[weak]
                    row_obj,
                    #[upgrade_or]
                    None,
                    move |values| {
                        let position = values[1].get::<f64>().unwrap_or(0.0);
                        let duration = row_obj.recording().duration();
                        if let Some(player) = widget.imp().player.borrow().as_ref() {
                            player.seek(gst::ClockTime::from_nseconds(
                                (position * duration as f64) as u64,
                            ));
                        }
                        None
                    }
                ),
            );

            let uri = recording.uri();
            row.connect_play(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |row, _uri| {
                    let imp = widget.imp();
                    let player = imp.player.borrow().clone();
                    if let Some(player) = player {
                        let active = imp.active_playing_row.borrow().clone();
                        match active {
                            Some(active) if active.as_ptr() != row.as_ptr() => {
                                active.set_state(RowState::Paused);
                                active.waveform().set_position_value(0.0);
                                player.set_uri(Some(&uri));
                            }
                            None => player.set_uri(Some(&uri)),
                            _ => {}
                        }
                        *imp.active_playing_row.borrow_mut() = Some(row.clone());
                        player.play();
                    }
                }
            ));

            row.connect_pause(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |_row| {
                    if let Some(player) = widget.imp().player.borrow().as_ref() {
                        player.pause();
                    }
                }
            ));

            row.connect_seek_backward(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |row| {
                    if let Some(player) = widget.imp().player.borrow().as_ref() {
                        seek_by(player, row, -10);
                    }
                }
            ));

            row.connect_seek_forward(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |row| {
                    if let Some(player) = widget.imp().player.borrow().as_ref() {
                        seek_by(player, row, 10);
                    }
                }
            ));

            row.connect_deleted(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |row| {
                    let imp = widget.imp();

                    let is_active = imp
                        .active_row
                        .borrow()
                        .as_ref()
                        .is_some_and(|active| active.as_ptr() == row.as_ptr());
                    if is_active {
                        *imp.active_row.borrow_mut() = None;
                    }

                    let is_playing = imp
                        .active_playing_row
                        .borrow()
                        .as_ref()
                        .is_some_and(|active| active.as_ptr() == row.as_ptr());
                    if is_playing {
                        *imp.active_playing_row.borrow_mut() = None;
                        if let Some(player) = imp.player.borrow().as_ref() {
                            player.stop();
                        }
                    }

                    let index = row.index();
                    imp.isolate_at(index, false);
                    imp.sync_window_actions();
                    imp.obj()
                        .emit_by_name::<()>("row-deleted", &[&row.recording(), &(index as u32)]);
                }
            ));
        }

        fn on_state_changed(&self, state: gst_player::PlayerState) {
            if state == gst_player::PlayerState::Stopped {
                if let Some(row) = self.active_playing_row.borrow().as_ref() {
                    // Leave the waveform where it stopped (e.g. at the end after a
                    // forward seek or EOS). Switching rows resets it explicitly.
                    row.set_state(RowState::Paused);
                }
            } else if state == gst_player::PlayerState::Playing {
                if let Some(row) = self.active_playing_row.borrow().as_ref() {
                    row.set_state(RowState::Playing);
                }
            }
        }

        fn on_position_updated(&self, position: Option<gst::ClockTime>) {
            if let Some(row) = self.active_playing_row.borrow().as_ref() {
                if let Some(position) = position {
                    let duration = row.recording().duration();
                    if duration > 0 {
                        row.waveform()
                            .set_position_value(position.nseconds() as f64 / duration as f64);
                    }
                }
            }
        }

        fn on_row_activated(&self, row: &Row) {
            let active_editing = self
                .active_row
                .borrow()
                .as_ref()
                .is_some_and(|active| active.edit_mode() && active.expanded());

            if (row.edit_mode() && row.expanded()) || active_editing {
                return;
            }

            if let Some(active) = self.active_row.borrow().as_ref() {
                if active.as_ptr() != row.as_ptr() {
                    active.set_expanded(false);
                    self.isolate_at(active.index(), false);
                }
            }

            row.set_expanded(!row.expanded());
            self.isolate_at(row.index(), row.expanded());

            *self.active_row.borrow_mut() = Some(row.clone());
            self.sync_window_actions();
        }

        /// Put the active row's `recording` action group on the toplevel so that
        /// application accelerators (`space`, `b`, `n`, …) reach it regardless of
        /// which widget currently has focus. Called whenever the active row or the
        /// list's visibility changes.
        pub fn sync_window_actions(&self) {
            let Some(window) = self
                .obj()
                .root()
                .and_then(|root| root.downcast::<gtk::Window>().ok())
            else {
                return;
            };

            let group = if self.shortcuts_active.get() {
                self.active_row
                    .borrow()
                    .as_ref()
                    .and_then(|row| row.action_group())
            } else {
                None
            };

            match group {
                Some(group) => window.insert_action_group("recording", Some(&group)),
                None => window.insert_action_group("recording", None::<&gio::SimpleActionGroup>),
            }
        }

        /// Toggle play/pause on the active row (used by the `win.play-pause` action).
        pub fn toggle_playback(&self) {
            // Always act on the active (expanded) row: if a different row is
            // playing, activating this row's `play` stops it and starts the
            // active one (handled by `connect_play`).
            let row = self
                .active_row
                .borrow()
                .clone()
                .or_else(|| self.active_playing_row.borrow().clone());
            if let Some(row) = row {
                if let Some(group) = row.action_group() {
                    for name in ["pause", "play"] {
                        if let Some(action) = group.lookup_action(name) {
                            if action.is_enabled() {
                                action.activate(None);
                                return;
                            }
                        }
                    }
                }
            }
        }

        /// Delete the active row (used by the `win.delete-recording` action).
        pub fn delete_active(&self) {
            if let Some(row) = self.active_row.borrow().clone() {
                if let Some(group) = row.action_group() {
                    if let Some(action) = group.lookup_action("delete") {
                        if action.is_enabled() {
                            action.activate(None);
                        }
                    }
                }
            }
        }

        pub fn set_shortcuts_active(&self, active: bool) {
            self.shortcuts_active.set(active);
            self.sync_window_actions();
        }

        fn isolate_at(&self, index: i32, expanded: bool) {
            let list = self.list.get();
            let before = list.row_at_index(index - 1);
            let current = list.row_at_index(index);
            let after = list.row_at_index(index + 1);

            for (row, class) in [
                (current, "expanded"),
                (before, "expanded-before"),
                (after, "expanded-after"),
            ] {
                if let Some(row) = row {
                    if expanded {
                        row.add_css_class(class);
                    } else {
                        row.remove_css_class(class);
                    }
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct RecordingsListWidget(ObjectSubclass<imp::RecordingsListWidget>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl RecordingsListWidget {
    pub fn setup(&self, model: &gio::ListStore, player: &gst_player::Player) {
        self.imp().setup(model, player);
    }

    pub fn list(&self) -> Option<gtk::ListBox> {
        Some(self.imp().list.get())
    }

    pub fn stop_editing(&self) {
        if let Some(active) = self.imp().active_row.borrow().as_ref() {
            if active.edit_mode() {
                active.set_edit_mode(false);
            }
        }
    }

    pub fn set_shortcuts_active(&self, active: bool) {
        self.imp().set_shortcuts_active(active);
    }

    pub fn toggle_playback(&self) {
        self.imp().toggle_playback();
    }

    pub fn delete_active(&self) {
        self.imp().delete_active();
    }

    pub fn connect_row_deleted<F: Fn(&Self, &Recording, u32) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("row-deleted", false, move |values| {
            let obj = values[0].get::<RecordingsListWidget>().unwrap();
            let recording = values[1].get::<Recording>().unwrap();
            let index = values[2].get::<u32>().unwrap();
            f(&obj, &recording, index);
            None
        })
    }
}

fn seek_by(player: &gst_player::Player, row: &Row, seconds: i64) {
    let position = player.position().unwrap_or(gst::ClockTime::ZERO).nseconds() as i64;
    let duration = row.recording().duration().max(0);
    let ten = 10_000_000_000i64;
    let candidate = if seconds < 0 {
        position - ten
    } else {
        position + ten
    };
    // Clamp at both ends (no wrap-around); identical while playing or paused.
    let target = candidate.clamp(0, duration);
    player.seek(gst::ClockTime::from_nseconds(target as u64));

    // `position-updated` doesn't fire while the player is paused, so reflect the
    // new position in the waveform immediately.
    if duration > 0 {
        row.waveform()
            .set_position_value(target as f64 / duration as f64);
    }
}
