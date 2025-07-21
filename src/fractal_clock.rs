use egui::{
    Color32, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2,
    containers::{CollapsingHeader, Frame},
    emath, pos2,
    widgets::Slider,
};
use std::{
    f32::consts::TAU,
    time::{Duration, Instant},
};

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
    #[serde(skip)]
    paint_time: Duration,
    branch_color: Color32,
    hand_color: Color32,
    rainbow_mode: bool,
    pub fullscreen: bool,
    pub transparent_background: bool,

    // Preallocated buffers for performance
    #[serde(skip)]
    nodes_buf1: Vec<Node>,
    #[serde(skip)]
    nodes_buf2: Vec<Node>,
    #[serde(skip)]
    shapes: Vec<Shape>,

    // Precomputed colors to avoid recalculation every frame
    #[serde(skip)]
    depth_colors: Vec<Color32>,
    #[serde(skip)]
    color_state: ColorState,
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Default)]
enum ColorState {
    #[default]
    Clean,
    Dirty,
}

#[derive(Clone, Copy, PartialEq)]
struct Node {
    pos: Pos2,
    dir: Vec2,
}

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

impl Default for FractalClock {
    fn default() -> Self {
        Self {
            paused: false,
            time: 0.0,
            zoom: 0.075,
            start_line_width: 5.0,
            depth: 14,
            length_factor: 1.0,
            luminance_factor: 1.0,
            width_factor: 0.6,
            line_count: 0,
            paint_time: Duration::ZERO,
            hand_color: Color32::WHITE,
            branch_color: Color32::from_rgb(115, 186, 37),
            transparent_background: true,
            fullscreen: false,
            rainbow_mode: true,
            color_state: ColorState::Dirty,

            // Preallocate buffers
            nodes_buf1: Vec::with_capacity(1 << 16),
            nodes_buf2: Vec::with_capacity(1 << 16),
            shapes: Vec::with_capacity(1 << 18),
            depth_colors: Vec::with_capacity(16),
        }
    }
}

impl FractalClock {
    pub fn update(&mut self, ctx: &egui::Context) {
        if !self.paused {
            self.time = seconds_since_midnight();
            ctx.request_repaint();
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let painter = Painter::new(
            ui.ctx().clone(),
            ui.layer_id(),
            ui.available_rect_before_wrap(),
        );

        let now = Instant::now();
        self.paint(&painter);
        self.paint_time = now.elapsed();

        ui.expand_to_include_rect(painter.clip_rect());

        Frame::popup(ui.style())
            .stroke(Stroke::NONE)
            .show(ui, |ui| {
                ui.set_max_width(270.0);
                CollapsingHeader::new("Settings").show(ui, |ui| self.options_ui(ui));
            });
    }

    fn precompute_colors(&mut self) {
        self.depth_colors.clear();
        let mut luminance = 0.7;

        for depth_index in 0..self.depth {
            luminance *= self.luminance_factor;

            let luminance_u8 = (255.0 * luminance).round() as u8;
            if luminance_u8 == 0 {
                break;
            }

            let color = if self.rainbow_mode {
                let hue = (depth_index as f32) / (self.depth as f32) * 360.0;
                egui::epaint::Hsva::new(hue / 360.0, 1.0, 1.0, 1.0).into()
            } else {
                Color32::from_rgb(
                    (self.branch_color.r() as f32 * luminance).round() as u8,
                    (self.branch_color.g() as f32 * luminance).round() as u8,
                    (self.branch_color.b() as f32 * luminance).round() as u8,
                )
            };

            self.depth_colors.push(color);
        }

        self.color_state = ColorState::Clean;
    }

    fn options_ui(&mut self, ui: &mut Ui) {
        match self.format_time() {
            Some(time_str) => ui.label(time_str),
            None => ui.label("Invalid time value"),
        };

        ui.label(format!("Painted line count: {}", self.line_count));
        ui.label(format!("{:.2?} / paint", self.paint_time));

        ui.checkbox(&mut self.paused, "Paused");
        ui.add(Slider::new(&mut self.zoom, 0.0..=1.0).text("zoom"));
        ui.add(Slider::new(&mut self.start_line_width, 0.0..=5.0).text("Start line width"));

        if ui
            .add(Slider::new(&mut self.depth, 0..=20).text("depth"))
            .changed()
        {
            self.mark_colors_dirty();
        }

        if ui
            .add(Slider::new(&mut self.length_factor, 0.0..=1.0).text("length factor"))
            .changed()
        {
            self.mark_colors_dirty();
        }

        if ui
            .add(Slider::new(&mut self.luminance_factor, 0.0..=1.0).text("luminance factor"))
            .changed()
        {
            self.mark_colors_dirty();
        }

        ui.add(Slider::new(&mut self.width_factor, 0.0..=1.0).text("width factor"));

        egui::Grid::new("color_settings_grid").show(ui, |ui| {
            ui.label("Branch color:");
            if ui.color_edit_button_srgba(&mut self.branch_color).changed() {
                self.mark_colors_dirty();
            }
            ui.end_row();
            ui.label("Hand color:");
            ui.color_edit_button_srgba(&mut self.hand_color);
            ui.end_row();
        });

        if ui.checkbox(&mut self.rainbow_mode, "Rainbow").changed() {
            self.mark_colors_dirty();
        }
        ui.checkbox(&mut self.fullscreen, "Fullscreen mode");
        ui.checkbox(&mut self.transparent_background, "Transparent background");

        egui::reset_button(ui, self, "Reset");

        ui.hyperlink_to(
            "Standalone version of this code",
            "https://github.com/emilk/egui/blob/main/crates/egui_demo_app/src/apps/fractal_clock.rs",
        );
    }

    pub fn mark_colors_dirty(&mut self) {
        self.color_state = ColorState::Dirty;
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
        if matches!(self.color_state, ColorState::Dirty) {
            self.precompute_colors();
        }

        let rect = painter.clip_rect();
        let to_screen = emath::RectTransform::from_to(
            Rect::from_center_size(Pos2::ZERO, rect.square_proportions() / self.zoom),
            rect,
        );

        self.shapes.clear();
        self.nodes_buf1.clear();
        self.nodes_buf2.clear();

        let mut line_count = 0;
        let hands = self.create_hands();
        let hand_rotors = self.calculate_hand_rotors(&hands);

        self.draw_hands(&hands, &to_screen, rect, &mut line_count);
        self.draw_fractal_branches(&hand_rotors, &to_screen, rect, &mut line_count);

        self.line_count = line_count;
        painter.extend(self.shapes.drain(..));
    }

    fn create_hands(&self) -> [Hand; 3] {
        let angle_from_period =
            |period| TAU * (self.time.rem_euclid(period) / period) as f32 - TAU / 4.0;

        [
            Hand::from_length_angle(self.length_factor, angle_from_period(60.0)), // Second
            Hand::from_length_angle(self.length_factor, angle_from_period(3600.0)), // Minute
            Hand::from_length_angle(0.5, angle_from_period(43200.0)),             // Hour
        ]
    }

    fn calculate_hand_rotors(&self, hands: &[Hand; 3]) -> [emath::Rot2; 2] {
        let hand_rotations = [
            hands[0].angle - hands[2].angle + TAU / 2.0,
            hands[1].angle - hands[2].angle + TAU / 2.0,
        ];

        [
            hands[0].length * emath::Rot2::from_angle(hand_rotations[0]),
            hands[1].length * emath::Rot2::from_angle(hand_rotations[1]),
        ]
    }

    fn draw_hands(
        &mut self,
        hands: &[Hand; 3],
        to_screen: &emath::RectTransform,
        rect: Rect,
        line_count: &mut usize,
    ) {
        let center = pos2(0.0, 0.0);
        let width = self.start_line_width;

        for (i, hand) in hands.iter().enumerate() {
            let end = center + hand.vec;

            let line = [to_screen * center, to_screen * end];
            if rect.intersects(Rect::from_two_pos(line[0], line[1])) {
                self.shapes
                    .push(Shape::line_segment(line, (width, self.hand_color)));
                *line_count += 1;
            }

            if i < 2 {
                self.nodes_buf1.push(Node {
                    pos: end,
                    dir: hand.vec,
                });
            }
        }
    }

    fn draw_fractal_branches(
        &mut self,
        hand_rotors: &[emath::Rot2; 2],
        to_screen: &emath::RectTransform,
        rect: Rect,
        line_count: &mut usize,
    ) {
        let mut current_nodes = &mut self.nodes_buf1;
        let mut next_nodes = &mut self.nodes_buf2;
        let mut width = self.start_line_width;

        for &color in self.depth_colors.iter() {
            next_nodes.clear();
            width *= self.width_factor;

            for &rotor in hand_rotors {
                for &node in current_nodes.iter() {
                    let new_dir = rotor * node.dir;
                    let new_node = Node {
                        pos: node.pos + new_dir,
                        dir: new_dir,
                    };

                    let line = [to_screen * node.pos, to_screen * new_node.pos];
                    if rect.intersects(Rect::from_two_pos(line[0], line[1])) {
                        self.shapes.push(Shape::line_segment(line, (width, color)));
                        *line_count += 1;
                    }

                    next_nodes.push(new_node);
                }
            }

            std::mem::swap(&mut current_nodes, &mut next_nodes);
        }
    }
}

fn seconds_since_midnight() -> f64 {
    use chrono::Timelike;
    let now = chrono::Local::now().time();
    now.num_seconds_from_midnight() as f64 + 1e-9 * now.nanosecond() as f64
}
