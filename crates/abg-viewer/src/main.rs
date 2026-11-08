// Release builds open without a console window behind them; debug builds keep
// it so panics stay visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! A window for looking at what the generator produces, and for changing it.
//!
//! White is wall, black is open space. Pick a level, pick a seed, look at it.
//! Click a grid square and that unit is replaced by a very similar one — the
//! mechanic the whole project is built around, with a slider for how similar
//! "similar" has to be.

use std::time::Instant;

use abg_gen::{Grid, Level, Nudges, Params, generate_with};

/// Drawn over the tiles to show where one unit of a given level ends and the
/// next begins. Amber because it has to stay visible against both black open
/// space and white wall.
const GRID_COLOUR: egui::Color32 = egui::Color32::from_rgb(255, 138, 40);

/// Outlines the unit changed by the last click.
const CLICKED_COLOUR: egui::Color32 = egui::Color32::from_rgb(90, 200, 255);

/// Below this many screen pixels between lines, a grid is a wash of colour
/// rather than a grid, so it is not drawn at all.
const MIN_GRID_SPACING: f32 = 3.0;

struct Viewer {
    level: Level,
    params: Params,
    /// Which level's boundaries to outline, and the unit size a click acts on.
    grid_level: Option<Level>,
    /// How many single-tile changes one click applies.
    difference: u32,
    nudges: Nudges,
    /// Bumped whenever the nudges change, so the picture is regenerated
    /// without having to compare the whole map.
    revision: u64,
    /// The unit the last click landed on: origin and side, in tiles.
    last_click: Option<(u32, u32, u32)>,

    shown: Option<(Level, Params, u64)>,
    grid: Grid,
    texture: Option<egui::TextureHandle>,
    stats: Stats,
    /// Screen pixels per tile the last time the tiles were drawn, so the
    /// control panel can say when a grid would be too dense to draw.
    last_scale: Option<f32>,
    rng: u64,
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
            difference: 1,
            nudges: Nudges::new(),
            revision: 0,
            last_click: None,
            shown: None,
            grid: Grid::solid(1),
            texture: None,
            stats: Stats::default(),
            last_scale: None,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }

    fn random(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn regenerate(&mut self, ctx: &egui::Context) {
        let started = Instant::now();
        self.grid = generate_with(self.level, &self.params, &self.nudges);
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

        self.shown = Some((self.level, self.params, self.revision));
    }

    /// A click landed on the tile at `(x, y)`: replace the unit containing it
    /// with a very similar one.
    fn change_unit_at(&mut self, x: u32, y: u32) {
        let Some(grid_level) = self.grid_level else {
            return;
        };
        let unit = grid_level.side();
        let (ox, oy) = ((x / unit) * unit, (y / unit) * unit);

        // Each step moves one cell to a legal neighbour of itself, a single
        // tile different. Scattering the steps over the unit rather than
        // spending them all in one place is what makes the result read as
        // "something is subtly off" instead of "that corner was rebuilt".
        for _ in 0..self.difference {
            let tx = ox + (self.random() % u64::from(unit)) as u32;
            let ty = oy + (self.random() % u64::from(unit)) as u32;
            self.nudges.push(tx, ty);
        }

        self.last_click = Some((ox, oy, unit));
        self.revision += 1;
    }

    /// Screen pixels between grid lines for `level`, or `None` before anything
    /// has been drawn.
    fn grid_spacing_pixels(&self, level: Level) -> Option<f32> {
        self.last_scale.map(|scale| level.side() as f32 * scale)
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
        ui.label(egui::RichText::new("Grid — click a square to change it").strong());
        ui.radio_value(&mut self.grid_level, None, "off (clicking disabled)");
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
        ui.label(egui::RichText::new("Difference per click").strong());
        ui.add(egui::Slider::new(&mut self.difference, 1..=32).text("tiles"));
        ui.label(
            egui::RichText::new(
                "How far the replacement is allowed to be from what was there. \
                 1 is the smallest change the space can make. Steps can undo \
                 each other, so this is an upper bound.",
            )
            .small()
            .weak(),
        );

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let changed = !self.nudges.is_empty();
            if ui
                .add_enabled(changed, egui::Button::new("Reset changes"))
                .clicked()
            {
                self.nudges.clear();
                self.last_click = None;
                self.revision += 1;
            }
            if changed {
                ui.label(
                    egui::RichText::new(format!(
                        "{} cells, {} steps",
                        self.nudges.touched_cells(),
                        self.nudges.total_steps()
                    ))
                    .small()
                    .weak(),
                );
            }
        });

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

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Connectivity").strong());
        ui.add(egui::Slider::new(&mut self.params.connectivity, 0..=255));
        ui.label(
            egui::RichText::new(
                "Corridors beyond the minimum. 0 is a perfect maze — exactly \
                 one route between any two places. 255 links every neighbour.",
            )
            .small()
            .weak(),
        );
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

            ui.label("changes");
            ui.label(format!(
                "{} bytes",
                // Two u32 coordinates and a u32 count per touched cell, which
                // is the whole cost of everything the player has disturbed.
                self.nudges.touched_cells() * 12
            ));
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

        let response = ui.allocate_rect(rect, egui::Sense::click());
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let x = ((pos.x - rect.min.x) / scale).floor().max(0.0) as u32;
            let y = ((pos.y - rect.min.y) / scale).floor().max(0.0) as u32;
            let last = self.grid.side().saturating_sub(1);
            self.change_unit_at(x.min(last), y.min(last));
        }

        let painter = ui.painter_at(rect);

        if let Some(grid_level) = self.grid_level {
            let spacing = grid_level.side() as f32 * scale;
            if spacing >= MIN_GRID_SPACING {
                let stroke = egui::Stroke::new(1.0f32, GRID_COLOUR);
                let mut offset = 0.0f32;
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

        if let Some((ox, oy, unit)) = self.last_click {
            let min = egui::pos2(
                rect.min.x + ox as f32 * scale,
                rect.min.y + oy as f32 * scale,
            );
            let span = unit as f32 * scale;
            painter.rect_stroke(
                egui::Rect::from_min_size(min, egui::vec2(span, span)),
                0.0f32,
                egui::Stroke::new(2.0f32, CLICKED_COLOUR),
                egui::StrokeKind::Inside,
            );
        }
    }
}

impl eframe::App for Viewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.shown != Some((self.level, self.params, self.revision)) {
            self.regenerate(ctx);
        }

        egui::SidePanel::left("controls")
            .resizable(false)
            .exact_width(300.0)
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
                            "White is wall, black is open. A change only ever \
                             replaces a cell with a legal neighbour of itself, \
                             so it cannot seal anything off — \"reachable\" \
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
            .with_inner_size([1180.0, 880.0])
            .with_min_inner_size([820.0, 620.0])
            .with_title("Action-Based-Generation — generator"),
        ..Default::default()
    };

    eframe::run_native(
        "abg-viewer",
        options,
        Box::new(|_cc| Ok(Box::new(Viewer::new()))),
    )
}
