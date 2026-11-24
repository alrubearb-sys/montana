mod args;
mod engine;
mod look;

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Key, Pos2, Rect, Sense, Stroke, TextEdit, vec2,
};
use walkers::sources::{OpenTopoMap, OpenTopoServer};
use walkers::{HeaderValue, HttpOptions, HttpTiles, Map, MapMemory, Position, lon_lat};

use crate::args::{Args, USAGE, parse_args};
use crate::engine::Place;
use crate::look::{
    BACKGROUND, BLUE, BODY, CAPTION, CREDIT, DIM, FOREGROUND, GAP, RED, ROW_HEIGHT, ROW_PADDING,
    SEE_THROUGH, mix, omarchy_look, popup_frame, translucent,
};

// The panel tells a saved map from a closed one by the exit code, and reads the
// name of the new area from the output.
const CANCELLED: u8 = 2;
const START_ZOOM: f64 = 14.0;
const PLACE_ZOOM: f64 = 14.0;

// What Save does: a new area, or new points and name for the saved area named here.
#[derive(Clone)]
enum Task {
    Create,
    Edit(String),
}

// Whether "No matches" can be told: only after a search that has come back.
enum Search {
    Idle,
    Running,
    Done,
}

enum Message {
    Found(Result<Vec<Place>, engine::Error>),
    Saved(Result<(), engine::Error>),
}

struct MapApp {
    task: Task,
    tiles: HttpTiles,
    memory: MapMemory,
    start: Position,
    summit: Option<Position>,
    base: Option<Position>,
    name: String,
    kind: Option<String>,
    typed: String,
    places: Vec<Place>,
    search: Search,
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
    let title = match &args {
        Args::Create { .. } => String::from("New area · Peakcast"),
        Args::Edit { area, .. } => format!("Edit · {area} · Peakcast"),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            // What Hyprland matches to float it, as the window it is on the Mac.
            .with_app_id("peakcast-map")
            .with_inner_size([900.0, 640.0]),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "peakcast map",
        options,
        Box::new(move |cc| {
            omarchy_look(&cc.egui_ctx);
            Ok(Box::new(MapApp::new(args, saved, cc.egui_ctx.clone())))
        }),
    )
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
            user_agent: Some(HeaderValue::from_static(concat!(
                "peakcast-map/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/alrubearb-sys/peakcast)"
            ))),
            ..HttpOptions::default()
        };
        let mut memory = MapMemory::default();
        // Only fails outside the zoom levels the map knows.
        let _ = memory.set_zoom(START_ZOOM);
        let (sender, receiver) = mpsc::channel();
        // A new area starts with no marks; a saved one with its own.
        let (task, start, summit, base, name) = match args {
            Args::Create { center } => (Task::Create, position(center), None, None, String::new()),
            Args::Edit { area, summit, base } => (
                Task::Edit(area.clone()),
                position(summit),
                Some(position(summit)),
                base.map(position),
                area,
            ),
        };
        Self {
            task,
            tiles: HttpTiles::with_options(OpenTopoMap(OpenTopoServer::A), options, ctx),
            memory,
            start,
            summit,
            base,
            name,
            kind: None,
            typed: String::new(),
            places: Vec::new(),
            search: Search::Idle,
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
                    self.search = Search::Done;
                    self.places = places;
                }
                Message::Saved(Ok(())) => {
                    self.saved.replace(Some(self.name.trim().to_string()));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Message::Found(Err(error)) => {
                    self.search = Search::Idle;
                    self.problem = error.to_string();
                }
                Message::Saved(Err(error)) => {
                    self.saving = false;
                    self.problem = error.to_string();
                }
            }
        }
    }

    // The engine runs as another process: these wait for it away from the window.
    fn search(&mut self, ctx: &egui::Context) {
        if matches!(self.search, Search::Running) || self.typed.trim().is_empty() {
            return;
        }
        self.search = Search::Running;
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
        let task = self.task.clone();
        let summit = degrees(summit);
        let base = self.base.map(degrees);
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let saved = match task {
                Task::Create => engine::create(&name, kind.as_deref(), summit, base),
                Task::Edit(area) => engine::move_area(&area, &name, summit, base),
            };
            let _ = sender.send(Message::Saved(saved));
            ctx.request_repaint();
        });
    }

    // For a new area, the place found also names it; the name can still be changed.
    fn go_to(&mut self, place: &Place) {
        self.memory
            .center_at(position((place.latitude, place.longitude)));
        let _ = self.memory.set_zoom(PLACE_ZOOM);
        if matches!(self.task, Task::Create) {
            self.name.clone_from(&place.name);
            self.kind = Some(place.kind.clone());
        }
        self.clear_results();
    }

    fn clear_results(&mut self) {
        self.places.clear();
        self.search = Search::Idle;
    }

    fn map(&mut self, ui: &mut egui::Ui) {
        let summit = &mut self.summit;
        let base = &mut self.base;
        Map::new(Some(&mut self.tiles), &mut self.memory, self.start)
            // A mouse wheel zooms; on the Mac the trackpad zooms with a pinch and
            // its two-finger scroll moves the map, as walkers does by default.
            .zoom_with_ctrl(cfg!(target_os = "macos"))
            .show(ui, |ui, response, projector, _memory| {
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
            });
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
                            self.clear_results();
                        }
                        if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                            self.search(ui.ctx());
                        }
                    });
                    match self.search {
                        Search::Running => note(ui, "Searching..."),
                        Search::Done if self.places.is_empty() => note(ui, "No matches"),
                        Search::Idle | Search::Done => {}
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
                            let field = ui.add(
                                TextEdit::singleline(&mut self.name)
                                    .hint_text("Name")
                                    .frame(egui::Frame::NONE)
                                    .desired_width(200.0),
                            );
                            // Enter in the name does what Save does, which waits for
                            // a summit and a name.
                            if field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter))
                            {
                                self.save(ui.ctx());
                            }
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
                self.clear_results();
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
        &place.detail,
        FontId::proportional(CAPTION),
        DIM,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
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

// The tiles are kept on disk, as OpenStreetMap's tile policy asks.
fn tile_cache() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("peakcast").join("tiles"))
}
