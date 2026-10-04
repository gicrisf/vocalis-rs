use std::cell::RefCell;
use std::sync::OnceLock;

use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::recorder::Recorder;
use crate::recording::Recording;
use crate::utils::format_time;
use crate::waveform::{WaveForm, WaveType};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum RecorderState {
    #[default]
    Recording,
    Paused,
    Stopped,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/app/drey/Vocalis/ui/recorder.ui")]
    pub struct RecorderWidget {
        #[template_child(id = "recorderBox")]
        pub recorder_box: TemplateChild<gtk::Box>,
        #[template_child(id = "playbackStack")]
        pub playback_stack: TemplateChild<gtk::Stack>,
        #[template_child(id = "recorderTimeBin")]
        pub recorder_time_bin: TemplateChild<adw::Bin>,
        #[template_child(id = "recorderTime")]
        pub recorder_time: TemplateChild<gtk::Label>,
        #[template_child(id = "pauseBtn")]
        pub pause_btn: TemplateChild<gtk::Button>,
        #[template_child(id = "resumeBtn")]
        pub resume_btn: TemplateChild<gtk::Button>,

        recorder: RefCell<Option<Recorder>>,
        waveform: RefCell<Option<WaveForm>>,
        pub actions_group: RefCell<Option<gio::SimpleActionGroup>>,
        start_action: RefCell<Option<gio::SimpleAction>>,
        pause_action: RefCell<Option<gio::SimpleAction>>,
        stop_action: RefCell<Option<gio::SimpleAction>>,
        resume_action: RefCell<Option<gio::SimpleAction>>,
        cancel_action: RefCell<Option<gio::SimpleAction>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecorderWidget {
        const NAME: &'static str = "VocalisRecorderWidget";
        type Type = super::RecorderWidget;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RecorderWidget {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("canceled").build(),
                    glib::subclass::Signal::builder("paused").build(),
                    glib::subclass::Signal::builder("resumed").build(),
                    glib::subclass::Signal::builder("started").build(),
                    glib::subclass::Signal::builder("stopped")
                        .param_types([Recording::static_type()])
                        .build(),
                ]
            })
        }
    }

    impl WidgetImpl for RecorderWidget {}
    impl BoxImpl for RecorderWidget {}

    impl RecorderWidget {
        pub fn setup(&self, recorder: &Recorder) {
            *self.recorder.borrow_mut() = Some(recorder.clone());

            let waveform = WaveForm::new(WaveType::Recorder);
            waveform.set_vexpand(true);
            waveform.set_valign(gtk::Align::Fill);
            self.recorder_box.prepend(&waveform);

            recorder
                .bind_property("current-peak", &waveform, "peak")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::DEFAULT)
                .build();

            let time_label = self.recorder_time.clone();
            recorder.connect_notify_local(Some("duration"), move |recorder, _| {
                time_label.set_markup(&format_time(recorder.duration()));
            });

            *self.waveform.borrow_mut() = Some(waveform);

            let group = gio::SimpleActionGroup::new();

            let start = gio::SimpleAction::new("start", None);
            start.set_enabled(true);
            start.connect_activate(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |_, _| {
                    widget.imp().on_start();
                }
            ));
            group.add_action(&start);

            let pause = gio::SimpleAction::new("pause", None);
            pause.connect_activate(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |_, _| {
                    widget.imp().on_pause();
                }
            ));
            group.add_action(&pause);

            let stop = gio::SimpleAction::new("stop", None);
            stop.connect_activate(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |_, _| {
                    widget.imp().on_stop();
                }
            ));
            group.add_action(&stop);

            let resume = gio::SimpleAction::new("resume", None);
            resume.connect_activate(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |_, _| {
                    widget.imp().on_resume();
                }
            ));
            group.add_action(&resume);

            let cancel = gio::SimpleAction::new("cancel", None);
            cancel.connect_activate(glib::clone!(
                #[weak(rename_to = widget)]
                self.obj(),
                move |_, _| {
                    widget.imp().on_cancel();
                }
            ));
            group.add_action(&cancel);

            start
                .bind_property("enabled", &cancel, "enabled")
                .flags(glib::BindingFlags::INVERT_BOOLEAN | glib::BindingFlags::SYNC_CREATE)
                .build();

            *self.start_action.borrow_mut() = Some(start);
            *self.pause_action.borrow_mut() = Some(pause);
            *self.stop_action.borrow_mut() = Some(stop);
            *self.resume_action.borrow_mut() = Some(resume);
            *self.cancel_action.borrow_mut() = Some(cancel);
            *self.actions_group.borrow_mut() = Some(group);

            // Initial action enablement: only "start" is available before recording.
            self.set_state(RecorderState::Stopped);
        }

        fn on_pause(&self) {
            self.playback_stack.set_visible_child_name("recorder-start");
            self.set_state(RecorderState::Paused);

            if let Some(recorder) = self.recorder.borrow().as_ref() {
                recorder.pause();
            }
            self.obj().emit_by_name::<()>("paused", &[]);
        }

        fn on_resume(&self) {
            self.playback_stack.set_visible_child_name("recorder-pause");
            self.set_state(RecorderState::Recording);

            if let Some(recorder) = self.recorder.borrow().as_ref() {
                recorder.resume();
            }
            self.obj().emit_by_name::<()>("resumed", &[]);
        }

        fn on_start(&self) {
            self.playback_stack.set_visible_child_name("recorder-pause");
            self.set_state(RecorderState::Recording);

            if let Some(recorder) = self.recorder.borrow().as_ref() {
                recorder.start();
            }
            self.obj().emit_by_name::<()>("started", &[]);
        }

        fn on_stop(&self) {
            self.set_state(RecorderState::Stopped);
            let recording = self
                .recorder
                .borrow()
                .as_ref()
                .and_then(|recorder| recorder.stop());
            if let Some(waveform) = self.waveform.borrow().as_ref() {
                waveform.cleanup();
            }
            match recording {
                Some(recording) => {
                    self.obj().emit_by_name::<()>("stopped", &[&recording]);
                }
                None => {
                    let null: Option<Recording> = None;
                    self.obj().emit_by_name::<()>("stopped", &[&null]);
                }
            }
        }

        fn on_cancel(&self) {
            self.on_pause();

            let dialog = adw::AlertDialog::builder()
                .heading(gettextrs::gettext("Delete Recording?"))
                .body(gettextrs::gettext("This recording will not be saved."))
                .build();
            dialog.add_response("close", &gettextrs::gettext("_Resume"));
            dialog.add_response("delete", &gettextrs::gettext("_Delete"));
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("close"));

            let widget = self.obj().clone();
            dialog.connect_response(Some("delete"), move |dialog, _| {
                let recorder = widget.imp().recorder.borrow().clone();
                widget.imp().set_state(RecorderState::Stopped);
                if let Some(recorder) = recorder {
                    if let Some(recording) = recorder.stop() {
                        recording.delete();
                    }
                }
                if let Some(waveform) = widget.imp().waveform.borrow().as_ref() {
                    waveform.cleanup();
                }
                widget.emit_by_name::<()>("canceled", &[]);
                dialog.close();
            });

            let widget = self.obj().clone();
            dialog.connect_response(Some("close"), move |dialog, _| {
                widget.imp().on_resume();
                dialog.close();
            });

            let parent = self
                .obj()
                .root()
                .and_then(|root| root.downcast::<gtk::Window>().ok());
            dialog.present(parent.as_ref());
        }

        fn set_state(&self, recorder_state: RecorderState) {
            let start = self.start_action.borrow().clone();
            let pause = self.pause_action.borrow().clone();
            let stop = self.stop_action.borrow().clone();
            let resume = self.resume_action.borrow().clone();

            match recorder_state {
                RecorderState::Paused => {
                    if let Some(action) = pause {
                        action.set_enabled(false);
                    }
                    if let Some(action) = resume {
                        action.set_enabled(true);
                    }
                    self.resume_btn.grab_focus();
                    self.recorder_time_bin.add_css_class("paused");
                }
                RecorderState::Recording => {
                    if let Some(action) = start {
                        action.set_enabled(false);
                    }
                    if let Some(action) = stop {
                        action.set_enabled(true);
                    }
                    if let Some(action) = resume {
                        action.set_enabled(false);
                    }
                    if let Some(action) = pause {
                        action.set_enabled(true);
                    }
                    self.pause_btn.grab_focus();
                    self.recorder_time_bin.remove_css_class("paused");
                }
                RecorderState::Stopped => {
                    if let Some(action) = start {
                        action.set_enabled(true);
                    }
                    if let Some(action) = stop {
                        action.set_enabled(false);
                    }
                    if let Some(action) = pause {
                        action.set_enabled(false);
                    }
                    if let Some(action) = resume {
                        action.set_enabled(false);
                    }
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct RecorderWidget(ObjectSubclass<imp::RecorderWidget>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl RecorderWidget {
    pub fn setup(&self, recorder: &Recorder) {
        self.imp().setup(recorder);
    }

    pub fn actions_group(&self) -> gio::SimpleActionGroup {
        self.imp().actions_group.borrow().clone().unwrap()
    }

    pub fn connect_started<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("started", false, move |values| {
            let obj = values[0].get::<RecorderWidget>().unwrap();
            f(&obj);
            None
        })
    }

    pub fn connect_canceled<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("canceled", false, move |values| {
            let obj = values[0].get::<RecorderWidget>().unwrap();
            f(&obj);
            None
        })
    }

    pub fn connect_stopped<F: Fn(&Self, Option<Recording>) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("stopped", false, move |values| {
            let obj = values[0].get::<RecorderWidget>().unwrap();
            let recording = values[1].get::<Option<Recording>>().ok().flatten();
            f(&obj, recording);
            None
        })
    }
}
