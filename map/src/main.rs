mod engine;

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontFamily, FontId, Key, Pos2, Rect, Sense,
    Stroke, TextEdit, vec2,
};
use eframe::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use walkers::sources::{OpenTopoMap, OpenTopoServer};
use walkers::{HeaderValue, HttpOptions, HttpTiles, Map, MapMemory, Position, lon_lat};

use crate::engine::Place;

// The panel tells a saved map from a closed one by the exit code, and reads the
// name of the new area from the output.
const CANCELLED: u8 = 2;
const START_ZOOM: f64 = 14.0;
const PLACE_ZOOM: f64 = 14.0;
const USAGE: &str = "usage: montana-map <latitude,longitude of where to start>";

// Omarchy's Tokyo Night roles, as in the panel.
const BACKGROUND: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x26);
const FOREGROUND: Color32 = Color32::from_rgb(0xa9, 0xb1, 0xd6);
const DIM: Color32 = Color32::from_rgb(0x56, 0x5f, 0x89);
const ACCENT: Color32 = Color32::from_rgb(0x7a, 0xa2, 0xf7);
const RED: Color32 = Color32::from_rgb(0xf7, 0x76, 0x8e);
const BLUE: Color32 = Color32::from_rgb(0x7a, 0xa2, 0xf7);

// Omarchy's type scale and spacing at its default base size of 12.
const CAPTION: f32 = 10.0;
// The map's credit must be there, but should hardly be seen.
const CREDIT: f32 = 9.0;
// Opacity of everything drawn over the map: the map is guessed through it, and the
// text stays easy to read.
const SEE_THROUGH: f32 = 0.88;
const BODY: f32 = 12.0;
const ROW_HEIGHT: f32 = 28.0;
const ROW_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;

struct Args {
    center: (f64, f64),
}

enum Message {
    Found(Result<Vec<Place>, engine::Error>),
    Saved(Result<(), engine::Error>),
}

// A new area: no marks until they are clicked, and a name to save it with.
struct MapApp {
    tiles: HttpTiles,
    memory: MapMemory,
    start: Position,
    summit: Option<Position>,
    base: Option<Position>,
    name: String,
    kind: Option<String>,
    typed: String,
    places: Vec<Place>,
    searched: bool,
    searching: bool,
    saving: bool,
    problem: String,
    sender: Sender<Message>,
    receiver: Receiver<Message>,
    saved: Rc<RefCell<Option<String>>>,
}

fn main() -> ExitCode {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let saved = Rc::new(RefCell::new(None));
    let result = run(args, Rc::clone(&saved));
    let name = saved.borrow_mut().take();
    match (result, name) {
        (Ok(()), Some(name)) => {
            println!("{name}");
            ExitCode::SUCCESS
        }
        (Ok(()), None) => ExitCode::from(CANCELLED),
        (Err(error), _) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args, saved: Rc<RefCell<Option<String>>>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("New area · montana")
            .with_inner_size([900.0, 640.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "montana map",
        options,
        Box::new(move |cc| {
            omarchy_look(&cc.egui_ctx);
            Ok(Box::new(MapApp::new(args, saved, cc.egui_ctx.clone())))
        }),
    )
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let center = args.next().ok_or("missing where to start")?;
    Ok(Args {
        center: coordinates(&center)?,
    })
}

fn coordinates(text: &str) -> Result<(f64, f64), String> {
    let invalid = || format!("expected latitude,longitude in degrees, got {text}");
    let (latitude, longitude) = text.split_once(',').ok_or_else(invalid)?;
    let latitude: f64 = latitude.trim().parse().map_err(|_| invalid())?;
    let longitude: f64 = longitude.trim().parse().map_err(|_| invalid())?;
    Ok((latitude, longitude))
}

fn position((latitude, longitude): (f64, f64)) -> Position {
    lon_lat(longitude, latitude)
}

fn degrees(position: Position) -> (f64, f64) {
    (position.y(), position.x())
}

impl MapApp {
    fn new(args: Args, saved: Rc<RefCell<Option<String>>>, ctx: egui::Context) -> Self {
        let options = HttpOptions {
            cache: tile_cache(),
            user_agent: Some(HeaderValue::from_static(
                "montana-map/0.1 (https://github.com/alrubearb-sys/montana)",
            )),
            ..HttpOptions::default()
        };
        let mut memory = MapMemory::default();
        // Only fails outside the zoom levels the map knows.
        let _ = memory.set_zoom(START_ZOOM);
        let (sender, receiver) = mpsc::channel();
        Self {
            tiles: HttpTiles::with_options(OpenTopoMap(OpenTopoServer::A), options, ctx),
            memory,
            start: position(args.center),
            summit: None,
            base: None,
            name: String::new(),
            kind: None,
            typed: String::new(),
            places: Vec::new(),
            searched: false,
            searching: false,
            saving: false,
            problem: String::new(),
            sender,
            receiver,
            saved,
        }
    }

    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                Message::Found(Ok(places)) => {
                    self.searching = false;
                    self.searched = true;
                    self.places = places;
                }
                Message::Saved(Ok(())) => {
                    self.saved.replace(Some(self.name.trim().to_string()));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Message::Found(Err(error)) | Message::Saved(Err(error)) => {
                    self.searching = false;
                    self.saving = false;
                    self.problem = error.to_string();
                }
            }
        }
    }

    // The engine runs as another process: these wait for it away from the window.
    fn search(&mut self, ctx: &egui::Context) {
        if self.searching || self.typed.trim().is_empty() {
            return;
        }
        self.searching = true;
        self.problem.clear();
        let typed = self.typed.clone();
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            // Only fails when the window is already gone.
            let _ = sender.send(Message::Found(engine::search(&typed)));
            ctx.request_repaint();
        });
    }

    fn ready(&self) -> bool {
        self.summit.is_some() && !self.name.trim().is_empty() && !self.saving
    }

    fn save(&mut self, ctx: &egui::Context) {
        let Some(summit) = self.summit.filter(|_| self.ready()) else {
            return;
        };
        self.saving = true;
        self.problem.clear();
        let name = self.name.trim().to_string();
        let kind = self.kind.clone();
        let summit = degrees(summit);
        let base = self.base.map(degrees);
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let created = engine::create(&name, kind.as_deref(), summit, base);
            let _ = sender.send(Message::Saved(created));
            ctx.request_repaint();
        });
    }

    // The place found also names the area, which can still be changed.
    fn go_to(&mut self, place: &Place) {
        self.memory
            .center_at(position((place.latitude, place.longitude)));
        let _ = self.memory.set_zoom(PLACE_ZOOM);
        self.name = place.name.clone();
        self.kind = Some(place.kind.clone());
        self.places.clear();
        self.searched = false;
    }

    fn map(&mut self, ui: &mut egui::Ui) {
        let summit = &mut self.summit;
        let base = &mut self.base;
        Map::new(Some(&mut self.tiles), &mut self.memory, self.start).show(
            ui,
            |ui, response, projector, _memory| {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let at = projector.unproject(pointer.to_vec2());
                    if response.clicked() {
                        *summit = Some(at);
                    } else if response.secondary_clicked() {
                        *base = Some(at);
                    }
                }
                let painter = ui.painter();
                if let Some(at) = *base {
                    mark(painter, projector.project(at).to_pos2(), "Base", BLUE);
                }
                if let Some(at) = *summit {
                    mark(painter, projector.project(at).to_pos2(), "Summit", RED);
                }
            },
        );
    }

    fn search_box(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::unique("search"))
            .anchor(Align2::LEFT_TOP, vec2(GAP * 2.0, GAP * 2.0))
            .show(ctx, |ui| {
                popup_frame().show(ui, |ui| {
                    ui.set_width(320.0);
                    ui.horizontal(|ui| {
                        ui.add_space(ROW_PADDING - GAP);
                        magnifier(ui);
                        let field = ui.add(
                            TextEdit::singleline(&mut self.typed)
                                .hint_text("Search")
                                .frame(egui::Frame::NONE)
                                .desired_width(f32::INFINITY),
                        );
                        if field.changed() {
                            self.places.clear();
                            self.searched = false;
                        }
                        if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                            self.search(ui.ctx());
                        }
                    });
                    if self.searching {
                        note(ui, "Searching...");
                    } else if self.searched && self.places.is_empty() {
                        note(ui, "No matches");
                    }
                    let mut chosen = None;
                    for place in &self.places {
                        if place_row(ui, place).clicked() {
                            chosen = Some(place.clone());
                        }
                    }
                    if let Some(place) = chosen {
                        self.go_to(&place);
                    }
                });
            });
    }

    fn buttons(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::unique("buttons"))
            // High enough to leave the corner to the map's credit.
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -GAP * 3.0))
            .show(ctx, |ui| {
                if !self.problem.is_empty() {
                    popup_frame().show(ui, |ui| {
                        ui.colored_label(RED, &self.problem);
                    });
                }
                ui.horizontal(|ui| {
                    // Drawn like the buttons beside it, so the three read as one row.
                    egui::Frame::NONE
                        .fill(translucent(mix(0.04), SEE_THROUGH))
                        .stroke(Stroke::new(1.0, translucent(FOREGROUND, 0.4)))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.add(
                                TextEdit::singleline(&mut self.name)
                                    .hint_text("Name")
                                    .frame(egui::Frame::NONE)
                                    .desired_width(200.0),
                            );
                        });
                    if ui.button("Cancel").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    let label = if self.saving { "Saving..." } else { "Save" };
                    if ui
                        .add_enabled(self.ready(), egui::Button::new(label))
                        .clicked()
                    {
                        self.save(ui.ctx());
                    }
                });
            });
    }
}

impl eframe::App for MapApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.receive(&ctx);
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            if self.places.is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.places.clear();
                self.searched = false;
            }
        }
        self.map(ui);
        self.search_box(&ctx);
        self.buttons(&ctx);
        egui::Area::new(egui::Id::unique("credits"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-4.0, -2.0))
            .show(&ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "© OpenStreetMap contributors, SRTM · © OpenTopoMap (CC-BY-SA)",
                    )
                    .size(CREDIT)
                    .color(translucent(BACKGROUND, 0.5)),
                );
            });
    }
}

// The panel's popups: background with a 2 px accent border and square corners.
fn popup_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(translucent(BACKGROUND, SEE_THROUGH))
        .stroke(Stroke::new(2.0, ACCENT))
        .inner_margin(4.0)
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.add_space(GAP / 2.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PADDING - GAP);
        ui.label(egui::RichText::new(text).color(DIM));
    });
}

// A row like the panel's area picker: name, and what and where it is below.
fn place_row(ui: &mut egui::Ui, place: &Place) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), ROW_HEIGHT + ROW_PADDING),
        Sense::click(),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, mix(0.08));
    }
    let left = rect.left() + ROW_PADDING;
    let painter = ui
        .painter()
        .with_clip_rect(rect.shrink2(vec2(ROW_PADDING, 0.0)));
    painter.text(
        Pos2::new(left, rect.center().y - 1.0),
        Align2::LEFT_BOTTOM,
        &place.name,
        FontId::proportional(BODY),
        FOREGROUND,
    );
    painter.text(
        Pos2::new(left, rect.center().y + 1.0),
        Align2::LEFT_TOP,
        detail(place),
        FontId::proportional(CAPTION),
        DIM,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// "peak · 2502 m · Schwende-Rüte, Switzerland", as in the panel.
fn detail(place: &Place) -> String {
    let kind = place.kind.replace('_', " ");
    let region = place
        .description
        .split_once(", ")
        .map_or(place.description.as_str(), |(_, rest)| rest);
    match place.elevation {
        Some(elevation) => format!("{kind} · {elevation:.0} m · {region}"),
        None => format!("{kind} · {region}"),
    }
}

// Drawn, not a glyph: the panel font has no icons.
fn magnifier(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(14.0, ROW_HEIGHT), Sense::hover());
    let center = rect.center() - vec2(1.5, 1.5);
    let stroke = Stroke::new(1.5, DIM);
    ui.painter().circle_stroke(center, 4.5, stroke);
    ui.painter()
        .line_segment([center + vec2(3.2, 3.2), center + vec2(6.5, 6.5)], stroke);
}

fn mark(painter: &egui::Painter, at: Pos2, label: &str, color: Color32) {
    painter.circle(at, 6.0, color, Stroke::new(2.0, BACKGROUND));
    let text = painter.layout_no_wrap(label.to_owned(), FontId::proportional(BODY), color);
    let corner = at + vec2(10.0, -text.size().y / 2.0);
    let background = Rect::from_min_size(corner, text.size()).expand(3.0);
    painter.rect_filled(background, CornerRadius::ZERO, BACKGROUND);
    painter.galley(corner, text, color);
}

fn translucent(color: Color32, alpha: f32) -> Color32 {
    let alpha = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

// The foreground over the background at Omarchy's fill strengths.
fn mix(alpha: f32) -> Color32 {
    let blend = |back: u8, front: u8| {
        let value = f32::from(back) + (f32::from(front) - f32::from(back)) * alpha;
        value.round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(
        blend(BACKGROUND.r(), FOREGROUND.r()),
        blend(BACKGROUND.g(), FOREGROUND.g()),
        blend(BACKGROUND.b(), FOREGROUND.b()),
    )
}

fn omarchy_look(ctx: &egui::Context) {
    ctx.add_font(FontInsert::new(
        "JetBrains Mono",
        FontData::from_static(include_bytes!(
            "../../bar/ui/fonts/JetBrainsMono-Regular.ttf"
        )),
        vec![
            InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Highest,
            },
            InsertFontFamily {
                family: FontFamily::Monospace,
                priority: FontPriority::Highest,
            },
        ],
    ));

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = BACKGROUND;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.override_text_color = Some(FOREGROUND);
    visuals.selection.bg_fill = mix(0.35);
    visuals.selection.stroke = Stroke::new(1.0, FOREGROUND);
    visuals.window_corner_radius = CornerRadius::ZERO;
    visuals.menu_corner_radius = CornerRadius::ZERO;
    visuals.window_stroke = Stroke::new(2.0, ACCENT);
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;
    // Save waits greyed until there is a summit and a name; at egui's 0.5, over the
    // see-through fill, it was hard to read.
    visuals.disabled_alpha = 0.7;
    // Omarchy's [controls]: fill and border strength for idle, hover and press. The
    // buttons sit on the map, so they let it show through.
    let widgets = &mut visuals.widgets;
    for (state, fill, border) in [
        (&mut widgets.inactive, 0.04, 0.4),
        (&mut widgets.hovered, 0.08, 0.25),
        (&mut widgets.active, 0.22, 0.25),
        (&mut widgets.open, 0.08, 0.25),
    ] {
        state.bg_fill = translucent(mix(fill), SEE_THROUGH);
        state.weak_bg_fill = translucent(mix(fill), SEE_THROUGH);
        state.bg_stroke = Stroke::new(1.0, translucent(FOREGROUND, border));
        state.fg_stroke = Stroke::new(1.0, FOREGROUND);
        state.corner_radius = CornerRadius::ZERO;
        state.expansion = 0.0;
    }
    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals_of(egui::Theme::Light, visuals);

    ctx.all_styles_mut(|style| {
        use egui::TextStyle;
        for (text_style, size) in [
            (TextStyle::Small, CAPTION),
            (TextStyle::Body, BODY),
            (TextStyle::Button, BODY),
            (TextStyle::Monospace, BODY),
            (TextStyle::Heading, 16.0),
        ] {
            style
                .text_styles
                .insert(text_style, FontId::proportional(size));
        }
        style.spacing.button_padding = vec2(10.0, 6.0);
        style.spacing.interact_size.y = ROW_HEIGHT;
        style.spacing.item_spacing = vec2(GAP, GAP / 2.0);
    });
}

// The tiles are kept on disk, as OpenStreetMap's tile policy asks.
fn tile_cache() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("montana").join("tiles"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|text| text.to_string()))
    }

    #[test]
    fn starts_where_it_is_told() {
        assert_eq!(args(&["47.4031,9.8132"]).unwrap().center, (47.4031, 9.8132));
        assert_eq!(args(&["40.4,-3.7"]).unwrap().center, (40.4, -3.7));
    }

    #[test]
    fn rejects_missing_or_bad_points() {
        assert!(args(&[]).is_err());
        assert!(args(&["north"]).is_err());
    }

    #[test]
    fn fills_mix_foreground_into_background() {
        assert_eq!(mix(0.0), BACKGROUND);
        assert_eq!(mix(1.0), FOREGROUND);
    }
}
