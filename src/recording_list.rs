use std::cell::RefCell;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::recording::Recording;

pub struct RecordingList {
    store: gio::ListStore,
    cancellable: gio::Cancellable,
    #[allow(dead_code)]
    monitor: gio::FileMonitor,
    enumerator: RefCell<Option<gio::FileEnumerator>>,
}

impl RecordingList {
    pub fn new() -> Self {
        let store = gio::ListStore::new::<Recording>();
        let cancellable = gio::Cancellable::new();

        let dir = config::recordings_dir();
        let monitor = dir
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, Some(&cancellable))
            .expect("failed to monitor recordings directory");

        let monitor_store = store.clone();
        monitor.connect_changed(move |_monitor, file1, _file2, event| {
            let index = index_of(&monitor_store, file1);
            match event {
                gio::FileMonitorEvent::MovedOut if index >= 0 => {
                    monitor_store.remove(index as u32);
                }
                gio::FileMonitorEvent::MovedIn if index == -1 => {
                    sorted_insert(&monitor_store, Recording::new(file1));
                }
                _ => {}
            }
        });

        let list = Self {
            store,
            cancellable,
            monitor,
            enumerator: RefCell::new(None),
        };
        list.enumerate_directory();

        list
    }

    pub fn store(&self) -> &gio::ListStore {
        &self.store
    }

    pub fn cancellable(&self) -> &gio::Cancellable {
        &self.cancellable
    }

    fn enumerate_directory(&self) {
        let store = self.store.clone();
        let dir = config::recordings_dir();
        let enumerator_slot = &self.enumerator;
        // The enumerator is stored once the future resolves, mirroring the TS cache.
        let _ = enumerator_slot;

        glib::MainContext::default().spawn_local(async move {
            let enumerator = match dir
                .enumerate_children_future(
                    "standard::name",
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    glib::Priority::LOW,
                )
                .await
            {
                Ok(enumerator) => enumerator,
                Err(err) => {
                    if !err.matches(gio::IOErrorEnum::Cancelled) {
                        eprintln!("Failed to load recordings {err}");
                    }
                    return;
                }
            };

            loop {
                match enumerator.next_files_future(5, glib::Priority::LOW).await {
                    Ok(infos) if infos.is_empty() => break,
                    Ok(infos) => {
                        for info in infos {
                            let file = dir.child(info.name());
                            sorted_insert(&store, Recording::new(&file));
                        }
                    }
                    Err(err) => {
                        if !err.matches(gio::IOErrorEnum::Cancelled) {
                            eprintln!("Failed to load recordings {err}");
                        }
                        break;
                    }
                }
            }

            let _ = enumerator.close(gio::Cancellable::NONE);
        });
    }
}

impl Default for RecordingList {
    fn default() -> Self {
        Self::new()
    }
}

fn index_of(store: &gio::ListStore, file: &gio::File) -> i32 {
    for i in 0..store.n_items() {
        if let Some(item) = store.item(i) {
            if let Ok(recording) = item.downcast::<Recording>() {
                if recording.uri() == file.uri() {
                    return i as i32;
                }
            }
        }
    }
    -1
}

fn sorted_insert(store: &gio::ListStore, recording: Recording) {
    for i in 0..store.n_items() {
        if let Some(item) = store.item(i) {
            if let Ok(current) = item.downcast::<Recording>() {
                if current
                    .time_modified()
                    .difference(&recording.time_modified())
                    .as_microseconds()
                    <= 0
                {
                    store.insert(i, &recording);
                    return;
                }
            }
        }
    }
    store.append(&recording);
}
