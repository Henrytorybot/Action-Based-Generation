// Release builds open without a console window behind them; debug builds keep
// it so panics stay visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! A window for looking at what the generator produces.
//!
//! White is wall, black is open space. Pick a level, pick a seed, look at it.
//! There is no player and nothing changes — this exists to answer one question:
//! does the output read as architecture, or as static?

use std::time::Instant;

use abg_gen::{Grid, Level, Params, generate};

/// Drawn over the tiles to show where one unit of a given level ends and the
/// next begins. Amber because it has to stay visible against both black open
/// space and white wall.
const GRID_COLOUR: egui::Color32 = egui::Color32::from_rgb(255, 138, 40);

/// Below this many screen pixels between lines, a grid is a wash of colour
/// rather than a grid, so it is not drawn at all.
const MIN_GRID_SPACING: f32 = 3.0;

struct Viewer {
    level: Level,
    params: Params,
    /// Which level's boundaries to outline, or `None` for no grid.
    grid_level: Option<Level>,
    /// What produced the grid currently on screen, so it is only regenerated
    /// when something that affects it actually changed.
    shown: Option<(Level, Params)>,
    grid: Grid,
    texture: Option<egui::TextureHandle>,
    stats: Stats,
    /// Screen pixels per tile the last time the tiles were drawn.
    ///
    /// Kept so the control panel can say when a grid would be too dense to
    /// draw, which depends on how big the window currently is.
    last_scale: Option<f32>,
}

#[derive(Default, Clone, Copy)]
struct Stats {
    micros: u128,
    open: u64,
    reachable: u64,
    bytes: usize,
}

impl Viewer {
    fn new() -> Self {
        Self {
            level: Level::House,
            params: Params::default(),
            grid_level: Some(Level::Cell),
            shown: None,
            grid: Grid::solid(1),
            texture: None,
            stats: Stats::default(),
            last_scale: None,
        }
    }

    fn regenerate(&mut self, ctx: &egui::Context) {
        let started = Instant::now();
        self.grid = generate(self.level, &self.params);
        let micros = started.elapsed().as_micros();

        self.stats = Stats {
            micros,
            open: self.grid.open_tiles(),
            reachable: self.grid.reachable_from_centre(),
            bytes: self.grid.bytes(),
        };

        let side = self.grid.side() as usize;
        let mut rgb = Vec::with_capacity(side * side * 3);
        for y in 0..self.grid.side() {
            for x in 0..self.grid.side() {
                let value = if self.grid.is_wall(x, y) { 255 } else { 0 };
                rgb.extend_from_slice(&[value, value, value]);
            }
        }

        let image = egui::ColorImage::from_rgb([side, side], &rgb);
        self.texture = Some(ctx.load_texture(
            "grid",
            image,
            // Nearest, or the tiles blur into each other the moment the image
            // is drawn at anything other than its exact pixel size.
            egui::TextureOptions::NEAREST,
        ));

        self.shown = Some((self.level, self.params));
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("Generator");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Unit").strong());
        for level in Level::ALL {
            let side = level.side();
            ui.radio_value(
                &mut self.level,
                level,
                format!("{}  ({side}×{side})", level.name()),
            );
        }

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Grid").strong());
        ui.radio_value(&mut self.grid_level, None, "off");
        for level in Level::ALL {
            // A grid coarser than the unit on screen would draw nothing but
            // the outer border, so those are left out.
            if level.depth() >= self.level.depth() {
                continue;
            }
            let dense = self
                .grid_spacing_pixels(level)
                .is_some_and(|px| px < MIN_GRID_SPACING);
            let label = if dense {
                format!("{}  (too dense to draw)", level.name())
            } else {
                level.name().to_string()
            };
            ui.radio_value(&mut self.grid_level, Some(level), label);
        }

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Seed").strong());
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.params.seed).speed(1.0));
            if ui.button("◀").clicked() {
                self.params.seed = self.params.seed.wrapping_sub(1);
            }
            if ui.button("▶").clicked() {
                self.params.seed = self.params.seed.wrapping_add(1);
            }
        });

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Openness").strong());
        ui.add(egui::Slider::new(&mut self.params.openness, 0..=255));
        ui.label(
            egui::RichText::new("How much space the cell alphabet leaves open.")
                .small()
                .weak(),
        );

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Connectivity").strong());
        ui.add(egui::Slider::new(&mut self.params.connectivity, 0..=255));
        ui.label(
            egui::RichText::new(
                "Corridors beyond the minimum. 0 is a perfect maze — exactly \
                 one route between any two places. 255 links every neighbour, \
                 so the unit becomes an open grid full of loops.",
            )
            .small()
            .weak(),
        );
    }

    /// Screen pixels between grid lines for `level`, or `None` if nothing has
    /// been drawn yet so the scale is not known.
    fn grid_spacing_pixels(&self, level: Level) -> Option<f32> {
        let scale = self.last_scale?;
        Some(level.side() as f32 * scale)
    }

    fn stats_table(&self, ui: &mut egui::Ui) {
        let side = self.level.side();
        let tiles = self.level.tiles();
        let stats = self.stats;
        let percent = if tiles > 0 {
            stats.open as f64 * 100.0 / tiles as f64
        } else {
            0.0
        };

        egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
            ui.label("size");
            ui.label(format!("{side} × {side}"));
            ui.end_row();

            ui.label("tiles");
            ui.label(format!("{tiles}"));
            ui.end_row();

            ui.label("open");
            ui.label(format!("{} ({percent:.1}%)", stats.open));
            ui.end_row();

            ui.label("memory");
            ui.label(format!("{} bytes", stats.bytes));
            ui.end_row();

            ui.label("generated in");
            ui.label(if stats.micros < 1000 {
                format!("{} µs", stats.micros)
            } else {
                format!("{:.2} ms", stats.micros as f64 / 1000.0)
            });
            ui.end_row();

            ui.label("reachable");
            let connected = stats.reachable == stats.open;
            ui.colored_label(
                if connected {
                    egui::Color32::from_rgb(130, 200, 130)
                } else {
                    egui::Color32::from_rgb(220, 110, 110)
                },
                if connected {
                    "all open space".to_string()
                } else {
                    format!("{} of {} — SEALED", stats.reachable, stats.open)
                },
            );
            ui.end_row();
        });
    }
}

impl Viewer {
    fn draw_tiles(&mut self, ui: &mut egui::Ui) {
        let Some(texture) = self.texture.clone() else {
            return;
        };

        let available = ui.available_rect_before_wrap();
        // Square, and snapped to a whole number of screen pixels per tile so
        // every tile is drawn the same size. Without the snap, a 243-wide grid
        // in a 500-pixel box gives some tiles two pixels and some one, which
        // reads as structure that is not in the data.
        let side = self.grid.side().max(1) as f32;
        let scale = (available.width().min(available.height()) / side)
            .floor()
            .max(1.0);
        self.last_scale = Some(scale);

        let size = egui::vec2(side * scale, side * scale);
        let rect = egui::Rect::from_center_size(available.center(), size);

        egui::Image::from_texture(egui::load::SizedTexture::new(texture.id(), size))
            .paint_at(ui, rect);

        let Some(grid_level) = self.grid_level else {
            return;
        };
        let spacing = grid_level.side() as f32 * scale;
        if spacing < MIN_GRID_SPACING {
            return;
        }

        let painter = ui.painter_at(rect);
        let stroke = egui::Stroke::new(1.0f32, GRID_COLOUR);
        let mut offset = 0.0;
        while offset <= size.x + 0.5 {
            let x = rect.min.x + offset;
            let y = rect.min.y + offset;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                stroke,
            );
            offset += spacing;
        }
    }
}

impl eframe::App for Viewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.shown != Some((self.level, self.params)) {
            self.regenerate(ctx);
        }

        egui::SidePanel::left("controls")
            .resizable(false)
            .exact_width(280.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.controls(ui);
                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);
                    self.stats_table(ui);
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(
                            "White is wall, black is open. Every open tile is \
                             reachable from the centre by design; \"reachable\" \
                             measures that rather than assuming it.",
                        )
                        .small()
                        .weak(),
                    );
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_tiles(ui);
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1140.0, 860.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Action-Based-Generation — generator"),
        ..Default::default()
    };

    eframe::run_native(
        "abg-viewer",
        options,
        Box::new(|_cc| Ok(Box::new(Viewer::new()))),
    )
}
