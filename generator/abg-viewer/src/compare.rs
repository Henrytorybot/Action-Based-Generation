//! The comparison tab: two units of the same size side by side, and every
//! sense in which they are alike.
//!
//! The right-hand unit can be built to order — "make me one that is 85% the
//! same as that" — which is the mutation mechanic asked for a result instead
//! of a number of steps.
//!
//! Distance is counted in **steps**, not shares. A step is one single thing:
//! one changed tile, one rotation, one mirror on an axis through the middle,
//! or one walk of a tile. All four are the same distance — a mirrored cell is
//! no further from its original than a cell with one tile moved — so the kind
//! of step is a footnote and the count is the measure.
//!
//! Only the first degree is counted. A cell more than one step away is not
//! slightly less familiar, it is somewhere else, so it is reported as being
//! past reach rather than folded into the total. When any cell is past reach
//! the answer becomes a floor, "at least n steps", rather than a number.
//!
//! The line to read it against is what two *unrelated* units come out at.
//! Nothing here starts at zero.

use std::time::Instant;

use abg_gen::{
    Grid, Kinds, Level, Nudges, Params, generate, generate_with,
    similarity::{Report, compare, nudges_for_tile_similarity},
};

/// How the right-hand unit is produced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RightHand {
    /// Built from the left one, aiming at a chosen similarity.
    Target,
    /// Generated from its own seed, with nothing to do with the left one.
    Unrelated,
}

/// Everything the picture depends on. When this changes, and only then, both
/// units are generated again.
#[derive(Clone, Copy, PartialEq)]
struct Stamp {
    level: Level,
    params: Params,
    /// The target as raw bits, so the stamp can be compared exactly.
    target: u32,
    hand: RightHand,
    unrelated_seed: u64,
    search_seed: u64,
    kinds: Kinds,
}

pub struct Compare {
    level: Level,
    params: Params,
    target: f32,
    hand: RightHand,
    unrelated_seed: u64,
    /// Which random order of cells the search pushes in. Changing it gives a
    /// different unit at the same similarity.
    search_seed: u64,
    /// Which kinds of change the search may use to get there.
    kinds: Kinds,

    left: Grid,
    right: Grid,
    report: Option<Report>,
    /// What the search actually reached, which is not always what was asked.
    reached: f32,
    /// The same comparison against a unit with no relation to the left
    /// one. Every figure on screen is read against this, because none of
    /// them starts at zero.
    floor: Option<Report>,
    nudges: Nudges,
    micros: u128,

    left_texture: Option<egui::TextureHandle>,
    right_texture: Option<egui::TextureHandle>,
    shown: Option<Stamp>,
}

impl Compare {
    pub fn new() -> Self {
        Self {
            level: Level::House,
            params: Params::default(),
            target: 90.0,
            hand: RightHand::Target,
            unrelated_seed: 999,
            search_seed: 1,
            kinds: Kinds::ALL,
            left: Grid::solid(1),
            right: Grid::solid(1),
            report: None,
            reached: 100.0,
            floor: None,
            nudges: Nudges::new(),
            micros: 0,
            left_texture: None,
            right_texture: None,
            shown: None,
        }
    }

    fn stamp(&self) -> Stamp {
        Stamp {
            level: self.level,
            params: self.params,
            target: self.target.to_bits(),
            hand: self.hand,
            unrelated_seed: self.unrelated_seed,
            search_seed: self.search_seed,
            kinds: self.kinds,
        }
    }

    pub fn refresh_if_needed(&mut self, ctx: &egui::Context) {
        let stamp = self.stamp();
        if self.shown == Some(stamp) {
            return;
        }

        let started = Instant::now();
        self.left = generate(self.level, &self.params);

        match self.hand {
            RightHand::Target => {
                let (nudges, reached) = nudges_for_tile_similarity(
                    self.level,
                    &self.params,
                    self.target,
                    self.search_seed,
                    self.kinds,
                );
                self.right = generate_with(self.level, &self.params, &nudges);
                self.nudges = nudges;
                self.reached = reached;
            }
            RightHand::Unrelated => {
                let params = Params {
                    seed: self.unrelated_seed,
                    ..self.params
                };
                self.right = generate(self.level, &params);
                self.nudges = Nudges::new();
                self.reached = f32::NAN;
            }
        }

        // The reference line. A seed nobody chose, so it stands for "no
        // relationship at all" rather than for any particular unit.
        let stranger = generate(
            self.level,
            &Params {
                seed: self.params.seed ^ 0x5DEE_CE66_D15C_0DE5,
                ..self.params
            },
        );
        self.floor = Some(compare(&self.left, &stranger));

        self.report = Some(compare(&self.left, &self.right));
        self.micros = started.elapsed().as_micros();

        self.left_texture = Some(texture(ctx, "compare-left", &self.left));
        self.right_texture = Some(texture(ctx, "compare-right", &self.right));
        self.shown = Some(stamp);
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("Compare");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Size").strong());
        for level in Level::ALL {
            let side = level.side();
            ui.radio_value(
                &mut self.level,
                level,
                format!("{}  ({side}×{side})", level.name()),
            );
        }

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Left — the place as it was").strong());
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.params.seed).speed(1.0));
            if ui.button("◀").clicked() {
                self.params.seed = self.params.seed.wrapping_sub(1);
            }
            if ui.button("▶").clicked() {
                self.params.seed = self.params.seed.wrapping_add(1);
            }
            ui.label(egui::RichText::new("seed").small().weak());
        });
        ui.add(egui::Slider::new(&mut self.params.openness, 0..=255).text("openness"));
        ui.add(egui::Slider::new(&mut self.params.connectivity, 0..=255).text("connectivity"));

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Right — what to put beside it").strong());
        ui.radio_value(&mut self.hand, RightHand::Target, "built to a similarity");
        ui.radio_value(&mut self.hand, RightHand::Unrelated, "an unrelated unit");

        ui.add_space(6.0);
        match self.hand {
            RightHand::Target => {
                ui.add(
                    egui::Slider::new(&mut self.target, 0.0f32..=100.0)
                        .suffix("%")
                        .text("aim for"),
                );
                ui.horizontal(|ui| {
                    if ui.button("Different one, same similarity").clicked() {
                        self.search_seed = self.search_seed.wrapping_add(1);
                    }
                });
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Kinds of change allowed").strong());
                ui.checkbox(&mut self.kinds.flip, "flip one tile");
                ui.checkbox(&mut self.kinds.turn, "turn a quarter");
                ui.checkbox(&mut self.kinds.mirror, "mirror");
                ui.checkbox(&mut self.kinds.slide, "slide, wrapping");
                if !self.kinds.any() {
                    ui.label(
                        egui::RichText::new("Nothing is allowed, so nothing will change.")
                            .small()
                            .color(egui::Color32::from_rgb(220, 160, 90)),
                    );
                }
                if self.kinds.slide {
                    ui.label(
                        egui::RichText::new(
                            "A slide moves the centre tile, and a cell with a walled \
                     centre is illegal. Those slides are refused rather than \
                     replaced, so about half of them do nothing.",
                        )
                        .small()
                        .weak(),
                    );
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!(
                        "reached {:.1}% — {} cells pushed, {} steps",
                        self.reached,
                        self.nudges.touched_cells(),
                        self.nudges.total_steps()
                    ))
                    .small()
                    .weak(),
                );
                if self.floor.is_some_and(|f| self.target < f.tiles) {
                    ui.label(
                        egui::RichText::new(
                            "Below the floor, so this is as far as pushing goes. \
                             Anything lower is not more different, only unrelated.",
                        )
                        .small()
                        .color(egui::Color32::from_rgb(220, 160, 90)),
                    );
                }
            }
            RightHand::Unrelated => {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.unrelated_seed).speed(1.0));
                    if ui.button("▶").clicked() {
                        self.unrelated_seed = self.unrelated_seed.wrapping_add(1);
                    }
                    ui.label(egui::RichText::new("seed").small().weak());
                });
            }
        }
    }

    pub fn results(&self, ui: &mut egui::Ui) {
        let Some(report) = self.report else {
            return;
        };

        ui.label(egui::RichText::new("How far apart they are").strong());
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(report.distance())
                .size(19.0)
                .strong()
                .color(if report.is_exact() {
                    egui::Color32::from_rgb(110, 195, 130)
                } else {
                    FLOOR_COLOUR
                }),
        );
        ui.label(
            egui::RichText::new(
                "A step is one single thing: one changed tile, one rotation, \
                 one mirror, or one walk of a tile. They are all the same \
                 distance.",
            )
            .small()
            .weak(),
        );

        ui.add_space(8.0);
        egui::Grid::new("steps")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                let row = |ui: &mut egui::Ui, label: &str, value: u64, dim: bool| {
                    let text = egui::RichText::new(label).small();
                    ui.label(if dim { text.weak() } else { text });
                    let number = egui::RichText::new(format!("{value}")).small().monospace();
                    ui.label(if dim { number.weak() } else { number });
                    ui.end_row();
                };

                row(ui, "cells in all", report.cells, true);
                row(ui, "already equal", report.equal, false);
                row(ui, "one step away", report.one_step, false);
                row(ui, "    by one changed tile", report.by_tile, true);
                row(ui, "    by one rotation", report.by_turn, true);
                row(ui, "    by one mirror", report.by_mirror, true);
                row(ui, "    by one walk", report.by_walk, true);
                row(ui, "past one step", report.beyond, false);
            });

        ui.label(
            egui::RichText::new(
                "The four lines under \"one step away\" overlap: a cell can be \
                 reachable more than one way, and is counted under each. They \
                 say what kind of step is available, not how many there are.",
            )
            .small()
            .weak(),
        );

        if report.beyond > 0 {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} cells are past one step, so the real distance is                      unknown and this is only a floor.",
                    report.beyond
                ))
                .small()
                .color(FLOOR_COLOUR),
            );
        }

        if let Some(floor) = self.floor.as_ref() {
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(format!("Two unrelated units: {}", floor.distance()))
                    .small()
                    .color(FLOOR_COLOUR),
            );
            ui.label(
                egui::RichText::new(
                    "Nothing here starts at zero, so that is the line to read \
                     the number above against.",
                )
                .small()
                .weak(),
            );
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Bulk measures").strong());
        ui.label(
            egui::RichText::new(
                "Whole-unit totals, as shares. None of these is a shape, so \
                 none is something a player could point at.",
            )
            .small()
            .weak(),
        );
        ui.add_space(6.0);

        egui::Grid::new("background")
            .num_columns(3)
            .spacing([8.0, 5.0])
            .show(ui, |ui| {
                for (index, (label, value)) in report.background().into_iter().enumerate() {
                    let base = self.floor.as_ref().map(|f| f.background()[index].1);
                    ui.label(egui::RichText::new(label).small());
                    ui.add(
                        egui::ProgressBar::new(value / 100.0)
                            .desired_width(70.0)
                            .fill(shade(value, base)),
                    );
                    ui.label(egui::RichText::new(format!("{value:.1}%")).small());
                    ui.end_row();
                }
            });

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!(
                "best whole-unit fit was with the right one {}",
                report.motion.name()
            ))
            .small()
            .weak(),
        );

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(if self.micros < 1000 {
                format!("built and compared in {} \u{00b5}s", self.micros)
            } else {
                format!(
                    "built and compared in {:.2} ms",
                    self.micros as f64 / 1000.0
                )
            })
            .small()
            .weak(),
        );
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let (Some(left), Some(right)) = (self.left_texture.clone(), self.right_texture.clone())
        else {
            return;
        };

        let available = ui.available_rect_before_wrap();
        let side = self.left.side().max(1) as f32;
        let gap = 16.0f32;
        let caption = 24.0f32;

        // Square, equal, and a whole number of screen pixels per tile, for the
        // same reason the other tab does it: a fractional scale invents
        // structure that is not in the data.
        let each = (available.width() - gap) / 2.0;
        let scale = (each.min(available.height() - caption) / side)
            .floor()
            .max(1.0);
        let size = egui::vec2(side * scale, side * scale);

        let span = egui::vec2(size.x * 2.0 + gap, size.y + caption);
        let top_left = egui::pos2(
            available.center().x - span.x / 2.0,
            available.center().y - span.y / 2.0 + caption,
        );

        let painter = ui.painter_at(available);
        let right_label = match self.hand {
            RightHand::Target => format!("right — {:.1}% the same", self.reached),
            RightHand::Unrelated => "right — unrelated".to_string(),
        };

        for (index, (texture, label)) in
            [(left, "left — as it was".to_string()), (right, right_label)]
                .into_iter()
                .enumerate()
        {
            let origin = egui::pos2(top_left.x + index as f32 * (size.x + gap), top_left.y);
            let rect = egui::Rect::from_min_size(origin, size);
            egui::Image::from_texture(egui::load::SizedTexture::new(texture.id(), size))
                .paint_at(ui, rect);
            painter.text(
                egui::pos2(rect.center().x, rect.min.y - 6.0),
                egui::Align2::CENTER_BOTTOM,
                label,
                egui::FontId::proportional(13.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
}

const FLOOR_COLOUR: egui::Color32 = egui::Color32::from_rgb(220, 160, 90);

/// Green once a figure stands clear of what chance alone would give, dull
/// otherwise. With no floor to read against, everything is dull: a bare
/// percentage here says nothing on its own.
fn shade(value: f32, floor: Option<f32>) -> egui::Color32 {
    match floor {
        Some(floor) if value > floor + (100.0 - floor) * 0.2 => {
            egui::Color32::from_rgb(110, 195, 130)
        }
        _ => egui::Color32::from_rgb(150, 150, 150),
    }
}

fn texture(ctx: &egui::Context, name: &str, grid: &Grid) -> egui::TextureHandle {
    let side = grid.side() as usize;
    let mut rgb = Vec::with_capacity(side * side * 3);
    for y in 0..grid.side() {
        for x in 0..grid.side() {
            let value = if grid.is_wall(x, y) { 255 } else { 0 };
            rgb.extend_from_slice(&[value, value, value]);
        }
    }
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgb([side, side], &rgb),
        egui::TextureOptions::NEAREST,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// egui runs without a window, so the tab's own logic can be exercised
    /// the same way the app exercises it.
    fn headless() -> egui::Context {
        egui::Context::default()
    }

    #[test]
    fn the_tab_builds_a_pair_and_compares_them() {
        let mut tab = Compare::new();
        tab.target = 85.0;
        tab.refresh_if_needed(&headless());

        let report = tab.report.expect("a comparison");
        assert_eq!(
            tab.left.side(),
            tab.right.side(),
            "the pair must match in size"
        );
        assert!(
            (report.tiles - tab.reached).abs() < 0.001,
            "the tab reports {} but the grids on screen score {}",
            tab.reached,
            report.tiles
        );
        assert!(tab.left_texture.is_some() && tab.right_texture.is_some());
    }

    #[test]
    fn every_size_produces_something() {
        let ctx = headless();
        let mut tab = Compare::new();
        tab.hand = RightHand::Unrelated;
        for level in Level::ALL {
            tab.level = level;
            tab.refresh_if_needed(&ctx);
            let report = tab.report.expect("a comparison");
            assert!(
                (0.0..=100.0).contains(&report.tiles),
                "{} scored {}",
                level.name(),
                report.tiles
            );
            // Only a unit at least one cell across has cells to compare.
            assert_eq!(report.cells > 0, level.side() >= 3, "{}", level.name());
        }
    }

    #[test]
    fn an_unrelated_pair_lands_on_the_floor_the_screen_quotes() {
        let mut tab = Compare::new();
        tab.hand = RightHand::Unrelated;
        tab.refresh_if_needed(&headless());

        let report = tab.report.expect("a comparison");
        let floor = tab.floor.expect("a floor to read against");
        assert!(
            (report.tiles - floor.tiles).abs() < 8.0,
            "an unrelated pair scored {} while the screen calls the floor {}",
            report.tiles,
            floor.tiles
        );
    }

    #[test]
    fn nothing_is_rebuilt_until_something_changes() {
        let ctx = headless();
        let mut tab = Compare::new();
        tab.refresh_if_needed(&ctx);
        let first = tab.micros;
        tab.micros = u128::MAX;
        tab.refresh_if_needed(&ctx);
        assert_eq!(
            tab.micros,
            u128::MAX,
            "the pair was rebuilt with nothing changed"
        );

        tab.params.seed += 1;
        tab.refresh_if_needed(&ctx);
        assert_ne!(tab.micros, u128::MAX, "changing the seed did not rebuild");
        let _ = first;
    }
}
