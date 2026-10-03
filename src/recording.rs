use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use std::time::Duration;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::recorder::ENCODING_PROFILES;

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::Recording)]
    pub struct Recording {
        #[property(get, set = Self::set_duration)]
        duration: Cell<i64>,
        #[property(get, set = Self::set_name)]
        name: RefCell<String>,
        file: RefCell<Option<gio::File>>,
        extension: RefCell<Option<String>>,
        time_modified: RefCell<Option<glib::DateTime>>,
        time_created: RefCell<Option<glib::DateTime>>,
        peaks: RefCell<Vec<f64>>,
        loaded_peaks: RefCell<Vec<f64>>,
        discoverer: RefCell<Option<gst_pbutils::Discoverer>>,
        pub pipeline: RefCell<Option<gst::Pipeline>>,
        bus_watch: RefCell<Option<gst::bus::BusWatchGuard>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Recording {
        const NAME: &'static str = "VocalisRecording";
        type Type = super::Recording;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Recording {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("peaks-updated").build(),
                    glib::subclass::Signal::builder("peaks-loading").build(),
                ]
            })
        }
    }

    impl Recording {
        pub fn init(&self, file: &gio::File) {
            let info = file
                .query_info(
                    "time::created,time::modified,standard::content-type",
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .ok();

            let (content_type, time_modified, time_created) = match &info {
                Some(info) => (
                    info.attribute_string("standard::content-type")
                        .map(|s| s.to_string()),
                    info.attribute_uint64("time::modified"),
                    info.attribute_uint64("time::created"),
                ),
                None => (None, 0, 0),
            };

            if let Some(content_type) = &content_type {
                for profile in ENCODING_PROFILES {
                    if profile.content_type == content_type {
                        *self.extension.borrow_mut() = Some(profile.extension.to_string());
                        break;
                    }
                }
            }

            let fallback = glib::DateTime::now_local().unwrap();
            *self.time_modified.borrow_mut() = Some(
                glib::DateTime::from_unix_local(time_modified as i64)
                    .unwrap_or_else(|_| fallback.clone()),
            );
            *self.time_created.borrow_mut() =
                Some(glib::DateTime::from_unix_local(time_created as i64).unwrap_or(fallback));

            let name = file
                .basename()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            self.name.replace(name);

            *self.file.borrow_mut() = Some(file.clone());

            self.start_discovery(file);
        }

        fn start_discovery(&self, file: &gio::File) {
            let discoverer = match gst_pbutils::Discoverer::new(gst::ClockTime::from_seconds(15)) {
                Ok(discoverer) => discoverer,
                Err(err) => {
                    eprintln!("Failed to create discoverer: {err}");
                    return;
                }
            };
            discoverer.start();

            let (sender, receiver) = std::sync::mpsc::channel::<Option<i64>>();
            discoverer.connect_discovered(move |_discoverer, info, error| {
                let duration = if error.is_none() {
                    info.duration().map(|duration| duration.nseconds() as i64)
                } else {
                    None
                };
                let _ = sender.send(duration);
            });

            let obj = self.obj().clone();
            glib::timeout_add_local(Duration::from_millis(50), move || {
                if let Ok(duration) = receiver.try_recv() {
                    if let Some(duration) = duration {
                        obj.set_property("duration", duration);
                    }
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            });

            if let Err(err) = discoverer.discover_uri_async(&file.uri()) {
                eprintln!("Failed to discover uri: {err}");
            }

            *self.discoverer.borrow_mut() = Some(discoverer);
        }

        pub fn file(&self) -> gio::File {
            self.file.borrow().clone().expect("recording has no file")
        }

        pub fn extension(&self) -> Option<String> {
            self.extension.borrow().clone()
        }

        pub fn time_modified(&self) -> glib::DateTime {
            self.time_modified
                .borrow()
                .clone()
                .unwrap_or_else(|| glib::DateTime::now_local().unwrap())
        }

        pub fn time_created(&self) -> glib::DateTime {
            self.time_created
                .borrow()
                .clone()
                .unwrap_or_else(|| glib::DateTime::now_local().unwrap())
        }

        pub fn set_duration(&self, value: i64) {
            self.duration.set(value);
            self.obj().notify("duration");
        }

        pub fn set_name(&self, value: String) {
            let _ = self.rename(&value);
        }

        pub fn rename(&self, value: &str) -> Result<(), glib::Error> {
            if value.is_empty() || value == self.name.borrow().as_str() {
                return Ok(());
            }
            let new_file = self
                .file()
                .set_display_name(value, gio::Cancellable::NONE)?;
            *self.file.borrow_mut() = Some(new_file);
            *self.name.borrow_mut() = value.to_string();
            self.obj().notify("name");
            Ok(())
        }

        pub fn peaks(&self) -> Vec<f64> {
            self.peaks.borrow().clone()
        }

        pub fn set_peaks(&self, data: Vec<f64>) {
            if data.is_empty() {
                return;
            }
            *self.peaks.borrow_mut() = data.clone();
            self.obj().emit_by_name::<()>("peaks-updated", &[]);

            let json = serde_json::to_vec(&data).expect("failed to serialize peaks");
            let cache = self.obj().waveform_cache();
            glib::MainContext::default().spawn_local(async move {
                let _ = cache
                    .replace_contents_future(
                        json,
                        None,
                        false,
                        gio::FileCreateFlags::REPLACE_DESTINATION,
                    )
                    .await;
            });
        }

        pub fn peaks_loading_and_generate(&self) {
            self.obj().emit_by_name::<()>("peaks-loading", &[]);
            self.generate_peaks();
        }

        pub fn generate_peaks(&self) {
            let element = match gst::parse::launch_full(
                "uridecodebin name=uridecodebin ! audioconvert ! audio/x-raw,channels=1 ! \
                 level name=level ! fakesink name=faked",
                None,
                gst::ParseFlags::empty(),
            ) {
                Ok(element) => element,
                Err(err) => {
                    eprintln!("Failed to build peaks pipeline: {err}");
                    return;
                }
            };
            let pipeline = match element.downcast::<gst::Pipeline>() {
                Ok(pipeline) => pipeline,
                Err(_) => {
                    eprintln!("Peaks pipeline is not a GstPipeline");
                    return;
                }
            };

            if let Some(uridecodebin) = pipeline.by_name("uridecodebin") {
                uridecodebin.set_property("uri", self.file().uri());
            }
            if let Some(fakesink) = pipeline.by_name("faked") {
                fakesink.set_property("qos", false);
                fakesink.set_property("sync", true);
            }

            if let Some(bus) = pipeline.bus() {
                let obj = self.obj().clone();
                let guard = bus
                    .add_watch_local(move |_, message| {
                        match message.view() {
                            gst::MessageView::Element(_) => {
                                if let Some(structure) = message.structure() {
                                    if structure.has_name("level") {
                                        if let Ok(peaks) = structure.get::<glib::ValueArray>("peak")
                                        {
                                            if let Some(value) = peaks.as_slice().first() {
                                                if let Ok(peak) = value.get::<f64>() {
                                                    obj.imp()
                                                        .loaded_peaks
                                                        .borrow_mut()
                                                        .push(10f64.powf(peak / 20.0));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            gst::MessageView::Eos(_) => {
                                let data = obj.imp().loaded_peaks.borrow().clone();
                                obj.imp().set_peaks(data);
                                let pipeline = obj.imp().pipeline.borrow().clone();
                                if let Some(pipeline) = pipeline {
                                    let _ = pipeline.set_state(gst::State::Null);
                                }
                                *obj.imp().pipeline.borrow_mut() = None;
                            }
                            _ => {}
                        }
                        glib::ControlFlow::Continue
                    })
                    .ok();
                *self.bus_watch.borrow_mut() = guard;
            }

            let _ = pipeline.set_state(gst::State::Playing);
            *self.pipeline.borrow_mut() = Some(pipeline);
        }
    }
}

glib::wrapper! {
    pub struct Recording(ObjectSubclass<imp::Recording>);
}

impl Recording {
    pub fn new(file: &gio::File) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().init(file);
        obj
    }

    pub fn file(&self) -> gio::File {
        self.imp().file()
    }

    pub fn uri(&self) -> String {
        self.imp().file().uri().to_string()
    }

    pub fn extension(&self) -> Option<String> {
        self.imp().extension()
    }

    pub fn time_modified(&self) -> glib::DateTime {
        self.imp().time_modified()
    }

    pub fn time_created(&self) -> glib::DateTime {
        self.imp().time_created()
    }

    pub fn peaks(&self) -> Vec<f64> {
        self.imp().peaks()
    }

    pub fn set_peaks(&self, data: Vec<f64>) {
        self.imp().set_peaks(data);
    }

    pub fn rename(&self, value: &str) -> Result<(), glib::Error> {
        self.imp().rename(value)
    }

    pub fn waveform_cache(&self) -> gio::File {
        config::cache_dir().child(format!("{}_data", self.name()))
    }

    pub fn load_peaks(&self) {
        let obj = self.clone();
        glib::MainContext::default().spawn_local(async move {
            let cache = obj.waveform_cache();
            match cache.load_bytes_future().await {
                Ok((bytes, _etag)) => match serde_json::from_slice::<Vec<f64>>(&bytes) {
                    Ok(data) => obj.imp().set_peaks(data),
                    Err(_) => obj.imp().peaks_loading_and_generate(),
                },
                Err(err) => {
                    eprintln!("Error reading waveform data file: {err}");
                    if err.matches(gio::IOErrorEnum::NotFound) {
                        obj.imp().peaks_loading_and_generate();
                    }
                }
            }
        });
    }

    pub fn stop_pipeline(&self) {
        let pipeline = self.imp().pipeline.borrow().clone();
        if let Some(pipeline) = pipeline {
            let _ = pipeline.set_state(gst::State::Null);
        }
        *self.imp().pipeline.borrow_mut() = None;
    }

    pub fn delete(&self) {
        let file = self.imp().file();
        let cache = self.waveform_cache();
        glib::MainContext::default().spawn_local(async move {
            let _ = file.trash_future(glib::Priority::HIGH).await;
            let _ = cache.trash_future(glib::Priority::DEFAULT).await;
        });
    }

    pub fn save(&self, dest: &gio::File) {
        let file = self.imp().file();
        let dest = dest.clone();
        glib::MainContext::default().spawn_local(async move {
            let (future, _progress) =
                file.copy_future(&dest, gio::FileCopyFlags::NONE, glib::Priority::DEFAULT);
            let _ = future.await;
        });
    }

    pub fn connect_peaks_updated<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("peaks-updated", false, move |values| {
            let obj = values[0].get::<Recording>().unwrap();
            f(&obj);
            None
        })
    }

    pub fn connect_peaks_loading<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("peaks-loading", false, move |values| {
            let obj = values[0].get::<Recording>().unwrap();
            f(&obj);
            None
        })
    }
}
