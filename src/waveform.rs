use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{cairo, gdk, glib};

const GUTTER: f64 = 4.0;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WaveType {
    #[default]
    Recorder,
    Player,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::WaveForm)]
    pub struct WaveForm {
        #[property(get = Self::position, set = Self::set_position)]
        pub position: Cell<f64>,
        #[property(get = Self::peak, set = Self::set_peak)]
        peak: Cell<f64>,
        pub peaks: RefCell<Vec<f64>>,
        pub last_x: Cell<Option<f64>>,
        wave_type: Cell<WaveType>,
        hc_id: RefCell<Option<glib::SignalHandlerId>>,
        accent_id: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WaveForm {
        const NAME: &'static str = "VocalisWaveForm";
        type Type = super::WaveForm;
        type ParentType = gtk::DrawingArea;
    }

    #[glib::derived_properties]
    impl ObjectImpl for WaveForm {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj().clone();
            self.obj().set_draw_func(move |_area, cr, width, height| {
                let imp = obj.imp();
                imp.draw(cr, width, height);
            });

            let style_manager = adw::StyleManager::default();
            let obj = self.obj().clone();
            let hc_id = style_manager.connect_high_contrast_notify(move |_| {
                obj.queue_draw();
            });
            *self.hc_id.borrow_mut() = Some(hc_id);

            let obj = self.obj().clone();
            let accent_id = style_manager.connect_accent_color_notify(move |_| {
                obj.queue_draw();
            });
            *self.accent_id.borrow_mut() = Some(accent_id);
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("position-changed")
                        .param_types([glib::Type::F64])
                        .build(),
                    glib::subclass::Signal::builder("gesture-pressed").build(),
                ]
            })
        }
    }

    impl WidgetImpl for WaveForm {}
    impl DrawingAreaImpl for WaveForm {}

    impl WaveForm {
        pub fn set_wave_type(&self, wave_type: WaveType) {
            self.wave_type.set(wave_type);
        }

        pub fn position(&self) -> f64 {
            let len = self.peaks.borrow().len() as f64;
            if len == 0.0 {
                0.0
            } else {
                -self.position.get() / (len * GUTTER)
            }
        }

        pub fn set_position(&self, pos: f64) {
            let value = self.clamped(-pos * self.peaks.borrow().len() as f64 * GUTTER);
            self.position.set(value);
            self.last_x.set(Some(value));
            self.obj().queue_draw();
            self.obj().notify("position");
        }

        fn peak(&self) -> f64 {
            self.peak.get()
        }

        fn set_peak(&self, p: f64) {
            let rounded = (p * 100.0).round() / 100.0;
            {
                let mut peaks = self.peaks.borrow_mut();
                let max_len = self.obj().width() as f64 / (2.0 * GUTTER);
                if peaks.len() as f64 > max_len {
                    peaks.pop();
                }
                peaks.insert(0, rounded);
            }
            self.peak.set(rounded);
            self.obj().queue_draw();
            self.obj().notify("peak");
        }

        pub fn set_peaks(&self, data: Vec<f64>) {
            *self.peaks.borrow_mut() = data;
            self.obj().queue_draw();
        }

        pub fn cleanup(&self) {
            self.peaks.borrow_mut().clear();
            self.obj().queue_draw();
        }

        pub fn clamped(&self, position: f64) -> f64 {
            let bound = -(self.peaks.borrow().len() as f64) * GUTTER;
            position.clamp(bound, 0.0)
        }

        #[allow(deprecated)]
        fn draw(&self, ctx: &cairo::Context, width: i32, _height: i32) {
            let area = self.obj();
            let max_height = area.height() as f64;
            let verti_center = max_height / 2.0;
            let horiz_center = area.width() as f64 / 2.0;

            let mut pointer = horiz_center + self.position.get();

            let style_manager = adw::StyleManager::default();

            let left_color = area.color();
            let right_alpha = if style_manager.is_high_contrast() {
                0.9
            } else {
                0.55
            };
            let right_color = gdk::RGBA::new(
                left_color.red(),
                left_color.green(),
                left_color.blue(),
                left_color.alpha() * right_alpha,
            );

            let divider_color = if self.wave_type.get() == WaveType::Player {
                style_manager
                    .accent_color()
                    .to_standalone_rgba(style_manager.is_dark())
            } else {
                area.style_context()
                    .lookup_color("destructive_color")
                    .unwrap_or_else(|| area.color())
            };

            ctx.set_line_cap(cairo::LineCap::Square);
            ctx.set_antialias(cairo::Antialias::None);
            ctx.set_line_width(1.0);

            set_source_rgba(ctx, divider_color);

            ctx.move_to(horiz_center, verti_center - max_height);
            ctx.line_to(horiz_center, verti_center + max_height);
            let _ = ctx.stroke();

            ctx.set_line_width(2.0);

            for peak in self.peaks.borrow().iter() {
                if pointer > width as f64 {
                    break;
                }

                if pointer > 0.0 {
                    if self.wave_type.get() == WaveType::Player && pointer > horiz_center {
                        set_source_rgba(ctx, right_color);
                    } else {
                        set_source_rgba(ctx, left_color);
                    }

                    ctx.move_to(pointer, verti_center + peak * max_height);
                    ctx.line_to(pointer, verti_center - peak * max_height);
                    let _ = ctx.stroke();
                }

                if self.wave_type.get() == WaveType::Player {
                    pointer += GUTTER;
                } else {
                    pointer -= GUTTER;
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct WaveForm(ObjectSubclass<imp::WaveForm>)
        @extends gtk::Widget, gtk::DrawingArea,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl WaveForm {
    pub fn new(wave_type: WaveType) -> Self {
        let obj: Self = glib::Object::new();
        obj.imp().set_wave_type(wave_type);

        if wave_type == WaveType::Player {
            obj.setup_drag_gesture();
        }

        obj
    }

    fn setup_drag_gesture(&self) {
        let gesture = gtk::GestureDrag::new();

        let obj = self.downgrade();
        gesture.connect_drag_begin(move |gesture, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            if let Some(obj) = obj.upgrade() {
                obj.emit_by_name::<()>("gesture-pressed", &[]);
            }
        });

        let obj = self.downgrade();
        gesture.connect_drag_update(move |_gesture, offset_x, _offset_y| {
            if let Some(obj) = obj.upgrade() {
                let imp = obj.imp();
                if let Some(last_x) = imp.last_x.get() {
                    let value = imp.clamped(offset_x + last_x);
                    imp.position.set(value);
                    obj.queue_draw();
                }
            }
        });

        let obj = self.downgrade();
        gesture.connect_drag_end(move |_gesture, _offset_x, _offset_y| {
            if let Some(obj) = obj.upgrade() {
                let imp = obj.imp();
                imp.last_x.set(Some(imp.position.get()));
                let position = imp.position();
                obj.emit_by_name::<()>("position-changed", &[&position]);
            }
        });

        self.add_controller(gesture);
    }

    pub fn set_peaks(&self, data: Vec<f64>) {
        self.imp().set_peaks(data);
    }

    pub fn cleanup(&self) {
        self.imp().cleanup();
    }

    pub fn set_position_value(&self, position: f64) {
        self.imp().set_position(position);
    }
}

fn set_source_rgba(ctx: &cairo::Context, rgba: gdk::RGBA) {
    ctx.set_source_rgba(
        rgba.red() as f64,
        rgba.green() as f64,
        rgba.blue() as f64,
        rgba.alpha() as f64,
    );
}
