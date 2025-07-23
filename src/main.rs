use crate::fractal_clock::FractalClock;

mod fractal_clock;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_transparent(true)
            .with_inner_size([1920.0, 1080.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Fractal Clock",
        options,
        Box::new(|cc| Ok(Box::new(WrapApp::new(cc)))),
    )
}

pub struct WrapApp {
    clock: FractalClock,
}

impl WrapApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            clock: eframe::get_value(cc.storage.expect("Storage error"), "fractal_clock")
                .unwrap_or_default(),
        }
    }
}

impl eframe::App for WrapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        self.clock.update(ctx);

        let frame = if self.clock.transparent_background {
            egui::Frame {
                fill: egui::Color32::TRANSPARENT,
                inner_margin: egui::Margin::ZERO,
                ..Default::default()
            }
        } else {
            egui::Frame::dark_canvas(&ctx.style())
        };

        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.clock.fullscreen));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            self.clock.ui(ui);
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "fractal_clock", &self.clock);
    }
}
