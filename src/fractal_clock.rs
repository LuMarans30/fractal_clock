use chrono::{DateTime, Local, Timelike};
use egui::{
    Color32, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2,
    containers::{CollapsingHeader, Frame},
    emath,
    epaint::Hsva,
    pos2,
    widgets::Slider,
};
use std::{
    f32::consts::TAU,
    time::{Duration, Instant},
};

// Configuration parameters
#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone)]
pub struct FractalClockConfig {
    zoom: f32,
    start_line_width: f32,
    depth: usize,
    length_factor: f32,
    luminance_factor: f32,
    width_factor: f32,
    branch_color: Color32,
    hand_color: Color32,
    rainbow_mode: bool,
    start_hsv: Hsva,
    end_hsv: Hsva,
}

impl Default for FractalClockConfig {
    fn default() -> Self {
        Self {
            zoom: 0.5,
            start_line_width: 5.0,
            depth: 15,
            length_factor: 0.75,
            luminance_factor: 1.0,
            width_factor: 0.75,
            branch_color: Color32::from_rgb(115, 186, 37),
            hand_color: Color32::WHITE,
            rainbow_mode: true,
            start_hsv: Hsva::new(0.0, 100.0, 100.0, 1.0),
            end_hsv: Hsva::new(240.0, 100.0, 100.0, 1.0),
        }
    }
}

#[derive(Default, PartialEq)]
struct FractalClockRendering {
    depth_colors: Vec<Color32>,
    nodes_buf1: Vec<Node>,
    nodes_buf2: Vec<Node>,
    shapes: Vec<Shape>,
}

impl FractalClockRendering {
    fn update_colors(&mut self, config: &FractalClockConfig) {
        const MIN_LUMINANCE: f32 = 0.5 / 255.0;
        self.depth_colors.clear();
        let mut luminance = 0.7;

        if config.rainbow_mode {
            for depth_index in 0..config.depth {
                luminance *= config.luminance_factor;
                if luminance < MIN_LUMINANCE {
                    break;
                }

                let t = depth_index as f32 / config.depth.max(1) as f32;

                let [h, s, v, a] = [
                    (config.start_hsv.h, config.end_hsv.h),
                    (config.start_hsv.s, config.end_hsv.s),
                    (config.start_hsv.v, config.end_hsv.v),
                    (config.start_hsv.a, config.end_hsv.a),
                ]
                .map(|(start, end)| egui::lerp(start..=end, t));

                self.depth_colors.push(Hsva::new(h, s, v, a).into());
            }
        } else {
            let [r, g, b, a] = config.branch_color.to_array().map(|c| c as f32 / 255.0);
            let multiply_color = |color: f32, factor: f32| (color * factor * 255.0).round() as u8;

            for _ in 0..config.depth {
                luminance *= config.luminance_factor;
                if luminance < MIN_LUMINANCE {
                    break;
                }
                let factor = luminance.min(1.0);

                let [r_new, g_new, b_new] = [r, g, b].map(|c| multiply_color(c, factor));
                let a_new = (a * 255.0).round() as u8;

                let color = Color32::from_rgba_premultiplied(r_new, g_new, b_new, a_new);
                self.depth_colors.push(color);
            }
        }
    }
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

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
pub struct FractalClock {
    paused: bool,
    #[serde(skip)]
    time: DateTime<Local>,
    config: FractalClockConfig,
    #[serde(skip)]
    line_count: usize,
    #[serde(skip)]
    paint_time: Duration,
    #[serde(skip)]
    rendering: FractalClockRendering,
    pub fullscreen: bool,
    pub transparent_background: bool,
}

impl Default for FractalClock {
    fn default() -> Self {
        Self {
            paused: false,
            time: Local::now(),
            config: FractalClockConfig::default(),
            line_count: 0,
            paint_time: Duration::ZERO,
            rendering: FractalClockRendering {
                depth_colors: Vec::with_capacity(16),
                nodes_buf1: Vec::with_capacity(1 << 16),
                nodes_buf2: Vec::with_capacity(1 << 16),
                shapes: Vec::with_capacity(1 << 18),
            },
            fullscreen: false,
            transparent_background: true,
        }
    }
}

impl FractalClock {
    pub fn update(&mut self, ctx: &egui::Context) {
        if !self.paused {
            self.time = Local::now();
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

    fn compute_colors(&mut self) {
        self.rendering.update_colors(&self.config);
    }

    fn options_ui(&mut self, ui: &mut Ui) {
        ui.label(self.time.format("%H:%M:%S:%S%.3f").to_string());
        ui.label(format!("Painted line count: {}", self.line_count));
        ui.label(format!("{:.2?} / paint", self.paint_time));

        ui.checkbox(&mut self.paused, "Paused");
        ui.add(Slider::new(&mut self.config.zoom, 0.0..=1.0).text("zoom"));
        ui.add(Slider::new(&mut self.config.start_line_width, 0.0..=5.0).text("Start line width"));

        if ui
            .add(Slider::new(&mut self.config.depth, 0..=20).text("depth"))
            .changed()
        {
            self.compute_colors();
        }
        if ui
            .add(Slider::new(&mut self.config.length_factor, 0.0..=1.0).text("length factor"))
            .changed()
        {
            self.compute_colors();
        }
        if ui
            .add(Slider::new(&mut self.config.luminance_factor, 0.0..=1.0).text("luminance factor"))
            .changed()
        {
            self.compute_colors();
        }

        ui.add(Slider::new(&mut self.config.width_factor, 0.0..=1.0).text("width factor"));

        egui::Grid::new("color_settings_grid").show(ui, |ui| {
            ui.label("Branch color:");
            if ui
                .color_edit_button_srgba(&mut self.config.branch_color)
                .changed()
            {
                self.compute_colors();
            }
            ui.end_row();
            ui.label("Hand color:");
            ui.color_edit_button_srgba(&mut self.config.hand_color);
            ui.end_row();
        });

        if ui
            .checkbox(&mut self.config.rainbow_mode, "Rainbow")
            .changed()
        {
            self.compute_colors();
        }

        if self.config.rainbow_mode {
            if ui
                .color_edit_button_hsva(&mut self.config.start_hsv)
                .changed()
            {
                self.compute_colors();
            }
            if ui
                .color_edit_button_hsva(&mut self.config.end_hsv)
                .changed()
            {
                self.compute_colors();
            }
        }

        ui.checkbox(&mut self.fullscreen, "Fullscreen mode");
        ui.checkbox(&mut self.transparent_background, "Transparent background");

        egui::reset_button(ui, self, "Reset");

        ui.hyperlink_to(
            "Standalone version of this code",
            "https://github.com/emilk/egui/blob/main/crates/egui_demo_app/src/apps/fractal_clock.rs",
        );
    }

    fn paint(&mut self, painter: &Painter) {
        if self.rendering.depth_colors.is_empty() {
            self.compute_colors();
        }

        let rect = painter.clip_rect();
        let to_screen = emath::RectTransform::from_to(
            Rect::from_center_size(Pos2::ZERO, rect.square_proportions() / self.config.zoom),
            rect,
        );

        self.rendering.shapes.clear();
        self.rendering.nodes_buf1.clear();
        self.rendering.nodes_buf2.clear();

        let mut line_count = 0;
        let hands = self.create_hands();
        let hand_rotors = self.calculate_hand_rotors(&hands);

        self.draw_hands(&hands, &to_screen, rect, &mut line_count);
        self.draw_fractal_branches(&hand_rotors, &to_screen, rect, &mut line_count);

        self.line_count = line_count;
        painter.extend(self.rendering.shapes.drain(..));
    }

    fn create_hands(&self) -> [Hand; 3] {
        let seconds = self.time.second() as f32 + self.time.nanosecond() as f32 / 1e9;
        let minutes = self.time.minute() as f32 + seconds / 60.0;
        let hours = self.time.hour() as f32 + minutes / 60.0;

        [
            Hand::from_length_angle(self.config.length_factor, TAU * seconds / 60.0 - TAU / 4.0),
            Hand::from_length_angle(self.config.length_factor, TAU * minutes / 60.0 - TAU / 4.0),
            Hand::from_length_angle(0.5, TAU * hours / 12.0 - TAU / 4.0),
        ]
    }

    fn calculate_hand_rotors(&self, hands: &[Hand; 3]) -> [emath::Rot2; 2] {
        let [second, minute, hour] = hands;
        let base_rotation = |hand: &Hand| {
            hand.length * emath::Rot2::from_angle(hand.angle - hour.angle + TAU / 2.0)
        };

        [base_rotation(second), base_rotation(minute)]
    }

    fn draw_hands(
        &mut self,
        hands: &[Hand; 3],
        to_screen: &emath::RectTransform,
        rect: Rect,
        line_count: &mut usize,
    ) {
        let center = pos2(0.0, 0.0);
        let screen_center = to_screen * center;
        let width = self.config.start_line_width;

        for (i, hand) in hands.iter().enumerate() {
            let end = center + hand.vec;
            let screen_end = to_screen * end;

            if rect.intersects(Rect::from_two_pos(screen_center, screen_end)) {
                self.rendering.shapes.push(Shape::line_segment(
                    [screen_center, screen_end],
                    (width, self.config.hand_color),
                ));
                *line_count += 1;
            }

            if i < 2 {
                self.rendering.nodes_buf1.push(Node {
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
        let mut current_nodes = &mut self.rendering.nodes_buf1;
        let mut next_nodes = &mut self.rendering.nodes_buf2;
        let mut width = self.config.start_line_width;

        for &color in self.rendering.depth_colors.iter() {
            next_nodes.clear();
            width *= self.config.width_factor;

            for &rotor in hand_rotors {
                for &node in current_nodes.iter() {
                    let new_dir = rotor * node.dir;
                    let new_node = Node {
                        pos: node.pos + new_dir,
                        dir: new_dir,
                    };

                    let line = [to_screen * node.pos, to_screen * new_node.pos];
                    if rect.intersects(Rect::from_two_pos(line[0], line[1])) {
                        self.rendering
                            .shapes
                            .push(Shape::line_segment(line, (width, color)));
                        *line_count += 1;
                    }

                    next_nodes.push(new_node);
                }
            }

            std::mem::swap(&mut current_nodes, &mut next_nodes);
        }
    }
}
