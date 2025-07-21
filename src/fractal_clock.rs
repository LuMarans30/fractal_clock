use egui::{
    Color32, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2,
    containers::{CollapsingHeader, Frame},
    emath, pos2,
    widgets::Slider,
};
use std::f32::consts::TAU;

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
pub struct FractalClock {
    paused: bool,
    time: f64,
    zoom: f32,
    start_line_width: f32,
    depth: usize,
    length_factor: f32,
    luminance_factor: f32,
    width_factor: f32,
    #[serde(skip)]
    line_count: usize,
    branch_color: Color32,
    hand_color: Color32,
    rainbow_mode: bool,
    pub fullscreen: bool,
    pub transparent_background: bool,
}

impl Default for FractalClock {
    fn default() -> Self {
        Self {
            paused: false,
            time: 0.0,
            zoom: 0.5,
            start_line_width: 5.0,
            depth: 14,
            length_factor: 0.75,
            luminance_factor: 1.0,
            width_factor: 0.75,
            line_count: 0,
            hand_color: Color32::WHITE,
            branch_color: Color32::from_rgb(115, 186, 37),
            transparent_background: true,
            fullscreen: false,
            rainbow_mode: false,
        }
    }
}

impl FractalClock {
    pub fn ui(&mut self, ui: &mut Ui) {
        if !self.paused {
            self.time = seconds_since_midnight();
            ui.ctx().request_repaint();
        }

        let painter = Painter::new(
            ui.ctx().clone(),
            ui.layer_id(),
            ui.available_rect_before_wrap(),
        );
        self.paint(&painter);
        ui.expand_to_include_rect(painter.clip_rect());

        Frame::popup(ui.style())
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                ui.set_max_width(270.0);
                CollapsingHeader::new("Settings").show(ui, |ui| self.options_ui(ui));
            });
    }

    fn options_ui(&mut self, ui: &mut Ui) {
        match self.format_time() {
            Some(time_str) => ui.label(time_str),
            None => ui.label("Invalid time value"),
        };

        ui.label(format!("Painted line count: {}", self.line_count));

        ui.checkbox(&mut self.paused, "Paused");
        ui.add(Slider::new(&mut self.zoom, 0.0..=1.0).text("zoom"));
        ui.add(Slider::new(&mut self.start_line_width, 0.0..=5.0).text("Start line width"));
        ui.add(Slider::new(&mut self.depth, 0..=15).text("depth"));
        ui.add(Slider::new(&mut self.length_factor, 0.0..=1.0).text("length factor"));
        ui.add(Slider::new(&mut self.luminance_factor, 0.0..=1.0).text("luminance factor"));
        ui.add(Slider::new(&mut self.width_factor, 0.0..=1.0).text("width factor"));

        egui::Grid::new("color_settings_grid").show(ui, |ui| {
            ui.label("Branch color:");
            ui.color_edit_button_srgba(&mut self.branch_color);
            ui.end_row();
            ui.label("Hand color:");
            ui.color_edit_button_srgba(&mut self.hand_color);
            ui.end_row();
        });

        ui.checkbox(&mut self.rainbow_mode, "Rainbow");
        ui.checkbox(&mut self.fullscreen, "Fullscreen mode");
        ui.checkbox(&mut self.transparent_background, "Transparent background");

        egui::reset_button(ui, self, "Reset");

        ui.hyperlink_to(
            "Standalone version of this code",
            "https://github.com/emilk/egui/blob/main/crates/egui_demo_app/src/apps/fractal_clock.rs",
        );
    }

    fn format_time(&self) -> Option<String> {
        use chrono::NaiveTime;

        let total_seconds = self.time.rem_euclid(86400.0);
        let secs = total_seconds as u32;
        let nanos = ((total_seconds.fract() * 1e9) as u32).min(999_999_999);

        NaiveTime::from_num_seconds_from_midnight_opt(secs, nanos)
            .map(|t| t.format("%H:%M:%S%.3f").to_string())
    }

    fn paint(&mut self, painter: &Painter) {
        struct Hand {
            length: f32,
            angle: f32,
            vec: Vec2,
        }

        impl Hand {
            fn from_length_angle(length: f32, angle: f32) -> Self {
                Self {
                    length,
                    angle,
                    vec: length * Vec2::angled(angle),
                }
            }
        }

        let angle_from_period =
            |period| TAU * (self.time.rem_euclid(period) / period) as f32 - TAU / 4.0;

        let hands = [
            Hand::from_length_angle(self.length_factor, angle_from_period(60.0)), // Second
            Hand::from_length_angle(self.length_factor, angle_from_period(3600.0)), // Minute
            Hand::from_length_angle(0.5, angle_from_period(43200.0)),             // Hour
        ];

        let rect = painter.clip_rect();
        let to_screen = emath::RectTransform::from_to(
            Rect::from_center_size(Pos2::ZERO, rect.square_proportions() / self.zoom),
            rect,
        );

        let mut shapes = Vec::new();
        let mut line_count = 0;

        let mut paint_line = |points: [Pos2; 2], color: Color32, width: f32| {
            let line = [to_screen * points[0], to_screen * points[1]];
            if rect.intersects(Rect::from_two_pos(line[0], line[1])) {
                shapes.push(Shape::line_segment(line, (width, color)));
                line_count += 1;
            }
        };

        let hand_rotations = [
            hands[0].angle - hands[2].angle + TAU / 2.0,
            hands[1].angle - hands[2].angle + TAU / 2.0,
        ];

        let hand_rotors = [
            hands[0].length * emath::Rot2::from_angle(hand_rotations[0]),
            hands[1].length * emath::Rot2::from_angle(hand_rotations[1]),
        ];

        #[derive(Clone, Copy)]
        struct Node {
            pos: Pos2,
            dir: Vec2,
        }

        let mut nodes = Vec::new();
        let mut width = self.start_line_width;

        // Draw main hands (white)
        for (i, hand) in hands.iter().enumerate() {
            let center = pos2(0.0, 0.0);
            let end = center + hand.vec;
            paint_line([center, end], self.hand_color, width);
            if i < 2 {
                nodes.push(Node {
                    pos: end,
                    dir: hand.vec,
                });
            }
        }

        let mut luminance = 0.7; // Start dimmer than main hands
        let mut new_nodes = Vec::new();

        // Draw fractal branches (green with depth-based darkening)
        for depth_index in 0..self.depth {
            new_nodes.clear();
            new_nodes.reserve(nodes.len() * 2);

            luminance *= self.luminance_factor;
            width *= self.width_factor;

            let luminance_u8 = (255.0 * luminance).round() as u8;
            if luminance_u8 == 0 {
                break;
            }

            let color = if self.rainbow_mode {
                // Cycle through hues based on depth index
                let hue = (depth_index as f32) / (self.depth as f32) * 360.0;
                egui::epaint::Hsva::new(hue / 360.0, 1.0, 1.0, 1.0).into()
            } else {
                // Original color calculation
                Color32::from_rgb(
                    (self.branch_color.r() as f32 * luminance).round() as u8,
                    (self.branch_color.g() as f32 * luminance).round() as u8,
                    (self.branch_color.b() as f32 * luminance).round() as u8,
                )
            };

            for &rotor in &hand_rotors {
                for a in &nodes {
                    let new_dir = rotor * a.dir;
                    let b = Node {
                        pos: a.pos + new_dir,
                        dir: new_dir,
                    };
                    paint_line([a.pos, b.pos], color, width);
                    new_nodes.push(b);
                }
            }

            std::mem::swap(&mut nodes, &mut new_nodes);
        }

        self.line_count = line_count;
        painter.extend(shapes);
    }
}

fn seconds_since_midnight() -> f64 {
    use chrono::Timelike;
    chrono::Local::now().time().num_seconds_from_midnight() as f64
        + 1e-9 * chrono::Local::now().time().nanosecond() as f64
}
