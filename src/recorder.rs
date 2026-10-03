use std::cell::{Cell, RefCell};

use gst::prelude::*;
use gst_pbutils::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config;

#[allow(dead_code)]
pub struct EncodingProfile {
    pub name: &'static str,
    pub container_caps: &'static str,
    pub audio_caps: &'static str,
    pub content_type: &'static str,
    pub extension: &'static str,
}

pub const ENCODING_PROFILES: &[EncodingProfile] = &[
    EncodingProfile {
        name: "VORBIS",
        container_caps: "application/ogg;audio/ogg;video/ogg",
        audio_caps: "audio/x-vorbis",
        content_type: "audio/x-vorbis+ogg",
        extension: "ogg",
    },
    EncodingProfile {
        name: "OPUS",
        container_caps: "application/ogg",
        audio_caps: "audio/x-opus",
        content_type: "audio/x-opus+ogg",
        extension: "opus",
    },
    EncodingProfile {
        name: "FLAC",
        container_caps: "audio/x-flac",
        audio_caps: "audio/x-flac",
        content_type: "audio/flac",
        extension: "flac",
    },
    EncodingProfile {
        name: "MP3",
        container_caps: "application/x-id3",
        audio_caps: "audio/mpeg,mpegversion=(int)1,layer=(int)3",
        content_type: "audio/mpeg",
        extension: "mp3",
    },
    EncodingProfile {
        name: "M4A",
        container_caps: "video/quicktime,variant=(string)iso",
        audio_caps: "audio/mpeg,mpegversion=(int)4",
        content_type: "video/mp4",
        extension: "m4a",
    },
];

struct AudioChannel {
    channels: i32,
}

const AUDIO_CHANNELS: &[AudioChannel] =
    &[AudioChannel { channels: 2 }, AudioChannel { channels: 1 }];

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Recorder)]
    pub struct Recorder {
        #[property(get, set = Self::set_duration)]
        duration: Cell<i64>,
        #[property(get, set = Self::set_current_peak)]
        current_peak: Cell<f64>,
        pub peaks: RefCell<Vec<f64>>,
        pub pipeline: RefCell<Option<gst::Pipeline>>,
        pub level: RefCell<Option<gst::Element>>,
        pub ebin: RefCell<Option<gst::Element>>,
        pub filesink: RefCell<Option<gst::Element>>,
        pub bus_watch: RefCell<Option<gst::bus::BusWatchGuard>>,
        pub timeout: RefCell<Option<glib::SourceId>>,
        pub file: RefCell<Option<gio::File>>,
        pub pipe_state: Cell<Option<gst::State>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recorder {
        const NAME: &'static str = "VocalisRecorder";
        type Type = super::Recorder;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Recorder {
        fn constructed(&self) {
            self.parent_constructed();
            self.build_pipeline();
        }
    }

    impl Recorder {
        fn build_pipeline(&self) {
            let pipeline = gst::Pipeline::new();

            let elements: Result<Vec<gst::Element>, _> =
                ["pulsesrc", "audioconvert", "level", "encodebin", "filesink"]
                    .iter()
                    .map(|factory| gst::ElementFactory::make(factory).build())
                    .collect();

            let elements = match elements {
                Ok(elements) => elements,
                Err(err) => {
                    eprintln!("Not all elements could be created: {err}");
                    return;
                }
            };

            for element in &elements {
                let _ = pipeline.add(element);
            }

            let caps: gst::Caps = "audio/x-raw".parse().expect("invalid caps");
            let _ = elements[0].link(&elements[1]);
            let _ = elements[1].link_filtered(&elements[2], &caps);

            *self.pipeline.borrow_mut() = Some(pipeline);
            *self.level.borrow_mut() = Some(elements[2].clone());
            *self.ebin.borrow_mut() = Some(elements[3].clone());
            *self.filesink.borrow_mut() = Some(elements[4].clone());
        }

        pub fn update_state(&self, state: Option<gst::State>) {
            self.pipe_state.set(state);
            if let Some(state) = state {
                if let Some(pipeline) = self.pipeline.borrow().as_ref() {
                    if pipeline.set_state(state).is_err() {
                        eprintln!("Unable to update the recorder pipeline state");
                    }
                }
            }
        }

        fn set_duration(&self, value: i64) {
            self.duration.set(value);
            self.obj().notify("duration");
        }

        fn set_current_peak(&self, mut peak: f64) {
            if peak > 0.0 {
                peak = 0.0;
            }
            let value = 10f64.powf(peak / 20.0);
            self.current_peak.set(value);
            self.peaks.borrow_mut().push(value);
            self.obj().notify("current-peak");
        }

        fn get_channel(&self) -> i32 {
            let index = config::settings().enum_("audio-channel") as usize;
            AUDIO_CHANNELS
                .get(index)
                .map(|channel| channel.channels)
                .unwrap_or(2)
        }

        pub fn get_profile(&self) -> Option<gst_pbutils::EncodingProfile> {
            let index = config::settings().enum_("audio-profile") as usize;
            let profile = ENCODING_PROFILES.get(index)?;

            let mut audio_caps: gst::Caps = profile.audio_caps.parse().ok()?;
            audio_caps
                .make_mut()
                .set_value("channels", self.get_channel().to_send_value());
            let audio_profile = gst_pbutils::EncodingAudioProfile::builder(&audio_caps)
                .presence(1)
                .build();

            let container_caps: gst::Caps = profile.container_caps.parse().ok()?;
            let container_profile = gst_pbutils::EncodingContainerProfile::builder(&container_caps)
                .name("record")
                .add_profile(audio_profile)
                .build();

            Some(container_profile.upcast())
        }

        pub fn on_message(&self, message: &gst::Message) {
            if gst_pbutils::MissingPluginMessage::is(message) {
                if let Ok(missing) = gst_pbutils::MissingPluginMessage::parse(message) {
                    eprintln!(
                        "Detail: {}\nDescription: {}",
                        missing.installer_detail(),
                        missing.description()
                    );
                }
                return;
            }

            match message.view() {
                gst::MessageView::Element(_) => {
                    if let Some(structure) = message.structure() {
                        if structure.has_name("level") {
                            if let Ok(peaks) = structure.get::<glib::ValueArray>("peak") {
                                if let Some(value) = peaks.as_slice().first() {
                                    if let Ok(peak) = value.get::<f64>() {
                                        self.set_current_peak(peak);
                                    }
                                }
                            }
                        }
                    }
                }
                gst::MessageView::Eos(_) => {
                    self.obj().stop();
                }
                gst::MessageView::Warning(warning) => {
                    eprintln!("{:?} {:?}", warning.error(), warning.debug());
                }
                gst::MessageView::Error(error) => {
                    eprintln!("{:?} {:?}", error.error(), error.debug());
                }
                _ => {}
            }
        }
    }
}

glib::wrapper! {
    pub struct Recorder(ObjectSubclass<imp::Recorder>);
}

impl Recorder {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn start(&self) {
        let imp = self.imp();

        let mut index = 1;
        let file = loop {
            let name = gettextrs::gettext("Recording %d").replace("%d", &index.to_string());
            index += 1;
            let Ok(candidate) = config::recordings_dir().child_for_display_name(&name) else {
                continue;
            };
            if !candidate.query_exists(gio::Cancellable::NONE) {
                break candidate;
            }
        };
        *imp.file.borrow_mut() = Some(file.clone());

        if let Some(pipeline) = imp.pipeline.borrow().as_ref() {
            if let Some(bus) = pipeline.bus() {
                let obj = self.clone();
                let guard = bus
                    .add_watch_local(move |_, message| {
                        obj.imp().on_message(message);
                        glib::ControlFlow::Continue
                    })
                    .ok();
                *imp.bus_watch.borrow_mut() = guard;
            }
        }

        let profile = imp.get_profile();
        if let (Some(ebin), Some(level), Some(filesink)) = (
            imp.ebin.borrow().clone(),
            imp.level.borrow().clone(),
            imp.filesink.borrow().clone(),
        ) {
            if let Some(profile) = profile {
                ebin.set_property("profile", &profile);
            }
            if let Some(path) = file.path() {
                filesink.set_property("location", path.to_string_lossy().to_string());
            }
            let _ = level.link(&ebin);
            let _ = ebin.link(&filesink);
        }

        imp.update_state(Some(gst::State::Playing));

        let obj = self.clone();
        let timeout = glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Some(pipeline) = obj.imp().pipeline.borrow().as_ref() {
                if let Some(position) = pipeline.query_position::<gst::ClockTime>() {
                    let nseconds = position.nseconds() as i64;
                    if nseconds > 0 {
                        obj.set_property("duration", nseconds);
                    }
                }
            }
            glib::ControlFlow::Continue
        });
        *imp.timeout.borrow_mut() = Some(timeout);
    }

    pub fn pause(&self) {
        self.imp().update_state(Some(gst::State::Paused));
    }

    pub fn resume(&self) {
        if self.imp().pipe_state.get() == Some(gst::State::Paused) {
            self.imp().update_state(Some(gst::State::Playing));
        }
    }

    pub fn stop(&self) -> Option<gio::File> {
        let imp = self.imp();

        imp.update_state(Some(gst::State::Null));
        self.set_property("duration", 0i64);

        if let Some(timeout) = imp.timeout.borrow_mut().take() {
            timeout.remove();
        }
        *imp.bus_watch.borrow_mut() = None;

        let file = imp.file.borrow().clone();
        if let Some(file) = file {
            if file.query_exists(gio::Cancellable::NONE) && !imp.peaks.borrow().is_empty() {
                imp.peaks.borrow_mut().clear();
                return Some(file);
            }
        }

        None
    }

    pub fn state(&self) -> Option<gst::State> {
        self.imp().pipe_state.get()
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an audio capture device"]
    fn records_to_file() {
        gst::init().expect("failed to init gstreamer");

        let recorder = Recorder::new();
        assert!(
            recorder.imp().get_profile().is_some(),
            "no encoding profile"
        );

        let main_loop = glib::MainLoop::new(None, false);
        let quit_loop = main_loop.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
            quit_loop.quit();
        });

        recorder.start();
        main_loop.run();

        let file = recorder.stop().expect("no recording produced");
        let path = file.path().expect("recording has no path");
        let metadata = std::fs::metadata(&path).expect("recording file missing");
        assert!(metadata.len() > 0, "recording file is empty");

        let _ = std::fs::remove_file(&path);
    }
}
