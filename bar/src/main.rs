mod clock;
mod engine;
mod place;
mod view;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::process::ExitCode;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{ComponentHandle, LogicalPosition, PlatformError, Timer};

use crate::engine::{Error, Place, SavedArea};

slint::slint! {
    #[edition = "1.19"]
    export { Panel, Tray } from "../ui/panel.slint";
}

const WIDTH: f64 = 440.0;
const MAX_AGE_SECONDS: i64 = 3600;
// A click on the icon takes the focus away first, so the panel is already closed by then.
const SAME_CLICK: Duration = Duration::from_millis(300);
// As often as Omarchy looks for its own updates, and only when the panel opens.
const CHECK_EVERY: Duration = Duration::from_hours(6);

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

struct App {
    // Only while it is open: a window takes several megabytes.
    panel: RefCell<Option<Panel>>,
    tray: Tray,
    areas: RefCell<Vec<String>>,
    // The same areas with their points, for the map.
    saved_areas: RefCell<Vec<SavedArea>>,
    selected: Cell<usize>,
    picker: Picker,
    reports: RefCell<HashMap<String, Saved>>,
    shown: RefCell<Shown>,
    // Counts the forecasts kept, so that each one can be told from the one before.
    stored: Cell<u64>,
    loading: RefCell<Option<String>>,
    problem: RefCell<String>,
    closed_at: Cell<Option<Instant>>,
    had_focus: Cell<bool>,
    // A newer release, from `peakcast outdated`; empty when this one is the newest.
    new_version: RefCell<String>,
    checked_at: Cell<Option<Instant>>,
}

// The area picker: saved areas that match what was typed, then places found.
#[derive(Default)]
struct Picker {
    typed: RefCell<String>,
    matches: RefCell<Vec<usize>>,
    found: RefCell<Vec<Place>>,
    searched: RefCell<String>,
    searching: Cell<bool>,
}

struct Saved {
    panel: engine::Panel,
    at: i64,
    revision: u64,
}

// What the window holds: setting a model again redraws the whole panel, even with
// the same content, so each is set only when it changes. A new window holds none.
#[derive(Default)]
struct Shown {
    // The area and the revision of its forecast, if there is one.
    forecast: Option<(String, Option<u64>)>,
    choices: Option<Vec<Choice>>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), PlatformError> {
    #[cfg(target_os = "macos")]
    if let Err(error) = start_at_login() {
        eprintln!("Error: cannot start Peakcast at login: {error}");
    }
    select_backend()?;
    let app = Rc::new(App {
        panel: RefCell::default(),
        tray: Tray::new()?,
        areas: RefCell::default(),
        saved_areas: RefCell::default(),
        selected: Cell::new(0),
        picker: Picker::default(),
        reports: RefCell::default(),
        shown: RefCell::default(),
        stored: Cell::new(0),
        loading: RefCell::default(),
        problem: RefCell::default(),
        closed_at: Cell::new(None),
        had_focus: Cell::new(false),
        new_version: RefCell::default(),
        checked_at: Cell::new(None),
    });
    app.tray.on_open(|| with_app(App::toggle));
    APP.with(|slot| slot.replace(Some(Rc::clone(&app))));

    app.tray.show()?;
    // Nothing runs while the panel is closed: the data is asked for when it opens.
    slint::run_event_loop_until_quit()
}

fn select_backend() -> Result<(), PlatformError> {
    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into());
    #[cfg(target_os = "macos")]
    let selector = selector.with_winit_event_loop_builder({
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        let mut builder = winit::event_loop::EventLoop::with_user_event();
        // No icon in the Dock.
        builder.with_activation_policy(ActivationPolicy::Accessory);
        builder
    });
    selector.select()
}

fn connect(panel: &Panel) {
    panel.on_previous(|| with_app(|app| app.move_selection(-1)));
    panel.on_next(|| with_app(|app| app.move_selection(1)));
    panel.on_choose(|index| {
        if let Ok(index) = usize::try_from(index) {
            with_app(|app| app.choose(index));
        }
    });
    panel.on_remove(|index| {
        if let Ok(index) = usize::try_from(index) {
            with_app(|app| app.remove(index));
        }
    });
    panel.on_move_area(|from, to| {
        if let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) {
            with_app(|app| app.move_area(from, to));
        }
    });
    panel.on_edit(|index| {
        if let Ok(index) = usize::try_from(index) {
            with_app(|app| app.edit(index));
        }
    });
    panel.on_edited(|text| with_app(|app| app.filter(&text)));
    panel.on_search(|text| with_app(|app| app.search(&text)));
    panel.on_refresh(|| with_app(App::fetch));
    panel.on_open_map(|| with_app(App::open_map));
    panel.on_update(|| with_app(App::update));
    panel.on_dismiss(|| with_app(App::close));
    panel.on_quit(|| {
        // It only fails when the loop has already ended.
        let _ = slint::quit_event_loop();
    });
    panel.window().on_winit_window_event(|_, event| {
        if let winit::event::WindowEvent::Focused(focused) = event {
            with_app(|app| app.focus_changed(*focused));
        }
        EventResult::Propagate
    });
}

fn with_app(action: impl FnOnce(&App)) {
    let app = APP.with(|slot| slot.borrow().clone());
    if let Some(app) = app {
        action(&app);
    }
}

impl App {
    fn toggle(&self) {
        let just_closed = self
            .closed_at
            .get()
            .is_some_and(|at| at.elapsed() < SAME_CLICK);
        if self.panel.borrow().is_some() {
            self.close();
        } else if !just_closed && let Err(error) = self.open() {
            eprintln!("Error: {error}");
        }
    }

    fn open(&self) -> Result<(), PlatformError> {
        let position = place::below_menu_bar(WIDTH);
        self.had_focus.set(false);
        let panel = Panel::new()?;
        connect(&panel);
        panel.show()?;
        if let Some((x, y)) = position {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "screen points fit in an f32"
            )]
            let position = LogicalPosition::new(x as f32, y as f32);
            panel.window().set_position(position);
        }
        self.panel.replace(Some(panel));
        self.shown.take();
        // The window exists for macOS only after the event loop has turned.
        Timer::single_shot(Duration::ZERO, || with_app(App::focus));
        self.show_selected();
        self.reload_areas(None);
        self.check_version();
        Ok(())
    }

    fn check_version(&self) {
        if self
            .checked_at
            .get()
            .is_some_and(|at| at.elapsed() < CHECK_EVERY)
        {
            return;
        }
        self.checked_at.set(Some(Instant::now()));
        in_background(engine::outdated, |app, version| {
            app.new_version.replace(version.unwrap_or_default());
            app.show_selected();
        });
    }

    // brew replaces the app while it is closed, and opens it again.
    fn update(&self) {
        match engine::update() {
            Ok(()) => {
                // It only fails when the loop has already ended.
                let _ = slint::quit_event_loop();
            }
            Err(error) => self.report_problem(&error),
        }
    }

    fn focus(&self) {
        if let Some(panel) = self.panel.borrow().as_ref() {
            place::activate();
            panel
                .window()
                .with_winit_window(|window| window.focus_window());
        }
    }

    // A new window starts by saying that it has no focus.
    fn focus_changed(&self, focused: bool) {
        if focused {
            self.had_focus.set(true);
        } else if self.had_focus.get() {
            self.close();
        }
    }

    fn close(&self) {
        let Some(panel) = self.panel.take() else {
            return;
        };
        self.closed_at.set(Some(Instant::now()));
        if let Err(error) = panel.hide() {
            eprintln!("Error: {error}");
        }
        // Later, because the panel may be the one that asked to be closed.
        Timer::single_shot(Duration::ZERO, move || drop(panel));
    }

    fn move_selection(&self, step: isize) {
        let total = self.areas.borrow().len();
        if total > 1 {
            let next = if step < 0 { total - 1 } else { 1 };
            self.select((self.selected.get() + next) % total);
        }
    }

    fn select(&self, index: usize) {
        if index < self.areas.borrow().len() {
            self.selected.set(index);
            self.problem.take();
            self.show_selected();
            self.refresh_if_old();
        }
    }

    fn selected_area(&self) -> Option<String> {
        self.areas.borrow().get(self.selected.get()).cloned()
    }

    fn filter(&self, typed: &str) {
        self.picker.typed.replace(typed.to_string());
        self.picker
            .matches
            .replace(view::matching(&self.areas.borrow(), typed));
        self.picker.found.borrow_mut().clear();
        self.picker.searched.borrow_mut().clear();
        self.show_choices();
    }

    fn search(&self, text: &str) {
        if self.picker.searching.get() {
            return;
        }
        self.picker.searching.set(true);
        self.picker.searched.replace(text.to_string());
        self.show_choices();
        let typed = text.to_string();
        let asked = text.to_string();
        in_background(
            move || engine::search(&typed),
            move |app, result| {
                app.picker.searching.set(false);
                // What was typed changed while it searched: these places answer
                // another text, and picking one would add the wrong place.
                if *app.picker.searched.borrow() != asked {
                    return;
                }
                match result {
                    Ok(places) => {
                        app.picker.found.replace(places);
                        app.problem.take();
                        app.show_selected();
                    }
                    Err(error) => app.report_problem(&error),
                }
            },
        );
    }

    fn choose(&self, index: usize) {
        let saved = self.picker.matches.borrow().get(index).copied();
        if let Some(area) = saved {
            self.select(area);
            return;
        }
        let number = index - self.picker.matches.borrow().len();
        let Some(place) = self.picker.found.borrow().get(number).cloned() else {
            return;
        };
        let known = self
            .areas
            .borrow()
            .iter()
            .position(|area| area.to_lowercase() == place.name.to_lowercase());
        if let Some(area) = known {
            self.select(area);
            return;
        }
        let text = self.picker.searched.borrow().clone();
        in_background(
            move || engine::add(&text, number + 1),
            move |app, result| match result {
                Ok(()) => app.reload_areas(Some(place.name)),
                Err(error) => app.report_problem(&error),
            },
        );
    }

    fn remove(&self, index: usize) {
        let Some(area) = self
            .picker
            .matches
            .borrow()
            .get(index)
            .and_then(|&area| self.areas.borrow().get(area).cloned())
        else {
            return;
        };
        in_background(
            move || engine::remove(&area),
            |app, result| match result {
                Ok(()) => app.reload_areas(None),
                Err(error) => app.report_problem(&error),
            },
        );
    }

    fn open_map(&self) {
        let Some(area) = self.selected_area() else {
            return;
        };
        // The map starts where the area on show is.
        let center = self
            .saved_areas
            .borrow()
            .iter()
            .find(|saved| saved.name == area)
            .map(|saved| saved.summit.clone());
        let Some(center) = center else {
            return;
        };
        in_background(
            move || engine::open_map(&center),
            |app, result| match result {
                Ok(Some(name)) => app.reload_areas(Some(name)),
                Ok(None) => {}
                Err(error) => app.report_problem(&error),
            },
        );
    }

    fn edit(&self, index: usize) {
        let area = self
            .picker
            .matches
            .borrow()
            .get(index)
            .and_then(|&area| self.areas.borrow().get(area).cloned())
            .and_then(|name| {
                self.saved_areas
                    .borrow()
                    .iter()
                    .find(|saved| saved.name == name)
                    .cloned()
            });
        let Some(area) = area else {
            return;
        };
        let old_name = area.name.clone();
        in_background(
            move || engine::edit_map(&area),
            move |app, result| match result {
                Ok(Some(name)) => {
                    // The points moved: the forecast kept is for the old ones.
                    app.reports.borrow_mut().remove(&old_name);
                    app.reload_areas(Some(name));
                }
                Ok(None) => {}
                Err(error) => app.report_problem(&error),
            },
        );
    }

    // The list takes the new order at once, so nothing jumps back while the engine
    // saves it; the area on show stays on show. Only an unfiltered list moves, so
    // `from` and `to` are places in the list of areas itself.
    fn move_area(&self, from: usize, to: usize) {
        let moved = {
            let mut areas = self.areas.borrow_mut();
            if from == to || from >= areas.len() || to >= areas.len() {
                return;
            }
            let shown = areas.get(self.selected.get()).cloned();
            let area = areas.remove(from);
            areas.insert(to, area.clone());
            let selected = areas.iter().position(|name| Some(name) == shown.as_ref());
            self.selected.set(selected.unwrap_or(0));
            area
        };
        self.show_selected();
        in_background(
            move || engine::order(&moved, to + 1),
            |app, result| match result {
                Ok(()) => app.reload_areas(None),
                Err(error) => app.report_problem(&error),
            },
        );
    }

    #[expect(
        clippy::unused_self,
        reason = "an action of the app like the others; the background task hands the app back"
    )]
    fn reload_areas(&self, wanted: Option<String>) {
        in_background(engine::areas, move |app, result| match result {
            Ok(saved) => {
                let areas: Vec<String> = saved.iter().map(|area| area.name.clone()).collect();
                app.saved_areas.replace(saved);
                let previous = wanted.or_else(|| app.selected_area());
                let index = areas
                    .iter()
                    .position(|area| Some(area) == previous.as_ref());
                app.picker
                    .matches
                    .replace(view::matching(&areas, &app.picker.typed.borrow()));
                app.areas.replace(areas);
                app.selected.set(index.unwrap_or(0));
                app.show_selected();
                app.refresh_if_old();
            }
            Err(error) => app.report_problem(&error),
        });
    }

    fn refresh_if_old(&self) {
        let fresh = self.selected_area().is_some_and(|area| {
            self.reports
                .borrow()
                .get(&area)
                .is_some_and(|saved| now() - saved.at < MAX_AGE_SECONDS)
        });
        if !fresh {
            self.fetch();
        }
    }

    fn fetch(&self) {
        let Some(area) = self.selected_area() else {
            return;
        };
        if self.loading.borrow().as_ref() == Some(&area) {
            return;
        }
        self.loading.replace(Some(area.clone()));
        self.show_selected();
        let name = area.clone();
        in_background(
            move || engine::panel(&name),
            move |app, result| app.store(&area, result),
        );
    }

    fn store(&self, area: &str, result: Result<engine::Panel, Error>) {
        if self.loading.borrow().as_deref() == Some(area) {
            self.loading.replace(None);
        }
        match result {
            Ok(panel) => {
                self.stored.set(self.stored.get() + 1);
                let saved = Saved {
                    panel,
                    at: now(),
                    revision: self.stored.get(),
                };
                self.reports.borrow_mut().insert(area.to_string(), saved);
                self.problem.take();
                self.show_selected();
            }
            Err(error) => self.report_problem(&error),
        }
    }

    fn report_problem(&self, error: &Error) {
        self.problem.replace(error.to_string());
        self.show_selected();
    }

    fn show_selected(&self) {
        let panel = self.panel.borrow();
        let Some(panel) = panel.as_ref() else {
            return;
        };
        panel.set_problem(self.problem.borrow().as_str().into());
        panel.set_new_version(self.new_version.borrow().as_str().into());
        let Some(area) = self.selected_area() else {
            return;
        };
        panel.set_area(area.as_str().into());

        let loading = self.loading.borrow().as_ref() == Some(&area);
        let reports = self.reports.borrow();
        let saved = reports.get(&area);
        let wanted = Some((area.clone(), saved.map(|saved| saved.revision)));
        if self.shown.borrow().forecast != wanted {
            match saved {
                Some(saved) => fill(panel, view::build(&saved.panel)),
                None => clear(panel),
            }
            self.shown.borrow_mut().forecast = wanted;
        }
        let updated = saved.map_or(String::new(), |saved| {
            let offset = saved.panel.utc_offset_seconds;
            format!("Updated {}", clock::clock(saved.at, offset))
        });
        let updated = if loading { "Updating" } else { &updated };
        panel.set_updated(updated.into());
        self.show_choices();
    }

    fn show_choices(&self) {
        let panel = self.panel.borrow();
        let Some(panel) = panel.as_ref() else {
            return;
        };
        let areas = self.areas.borrow();
        let matches = self.picker.matches.borrow();
        let choices = view::choices(&areas, &matches, &self.picker.found.borrow());
        if self.shown.borrow().choices.as_ref() != Some(&choices) {
            panel.set_choices(view::model(choices.clone()));
            self.shown.borrow_mut().choices = Some(choices);
        }
        let selected = matches
            .iter()
            .position(|&area| area == self.selected.get())
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1);
        panel.set_selected(selected);
        panel.set_searching(self.picker.searching.get());
        // Saved areas are dragged only while nothing is typed: a filtered list does
        // not show their real order.
        let movable = if self.picker.typed.borrow().trim().is_empty() && matches.len() > 1 {
            matches.len()
        } else {
            0
        };
        panel.set_movable(i32::try_from(movable).unwrap_or(0));
    }
}

fn fill(panel: &Panel, view: view::View) {
    panel.set_daylight(view.daylight.into());
    panel.set_snow_line(view.snow_line.into());
    panel.set_freezing_level(view.freezing_level.into());
    panel.set_snow_depth(view.snow_depth.into());
    panel.set_spots(view::model(view.spots));
    panel.set_days(view::model(view.days));
    panel.set_marks(view::model(view.marks));
    panel.set_temperature(view.temperature);
    panel.set_wind(view.wind);
    panel.set_precipitation(view.precipitation);
    panel.set_readings(view::model(view.readings));
    panel.set_version(view.version.into());
}

fn clear(panel: &Panel) {
    panel.set_daylight("".into());
    panel.set_snow_line("-".into());
    panel.set_freezing_level("-".into());
    panel.set_snow_depth("-".into());
    panel.set_spots(view::model(Vec::new()));
    panel.set_days(view::model(Vec::new()));
    panel.set_marks(view::model(Vec::new()));
    panel.set_temperature(Chart::default());
    panel.set_wind(Chart::default());
    panel.set_precipitation(Chart::default());
    panel.set_readings(view::model(Vec::new()));
    panel.set_version("".into());
}

fn in_background<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(&App, T) + Send + 'static,
) {
    thread::spawn(move || {
        let result = work();
        // It only fails when the app is closing.
        let _ = slint::invoke_from_event_loop(move || with_app(|app| done(app, result)));
    });
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or(0)
}

// Installed in /Applications, the app starts itself at login from then on: the
// first time it runs it writes a LaunchAgent that opens it. Copies run from
// elsewhere, as while developing, leave login alone.
#[cfg(target_os = "macos")]
fn start_at_login() -> std::io::Result<()> {
    use std::{env, fs, path::Path};

    const APP: &str = "/Applications/Peakcast.app";
    const AGENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>local.peakcast.bar</string>
  <key>ProgramArguments</key><array><string>/usr/bin/open</string><string>-g</string><string>/Applications/Peakcast.app</string></array>
  <key>RunAtLoad</key><true/>
  <key>LimitLoadToSessionType</key><string>Aqua</string>
</dict>
</plist>
"#;

    let Some(home) = env::var_os("HOME") else {
        return Ok(());
    };
    let agent = Path::new(&home).join("Library/LaunchAgents/local.peakcast.bar.plist");
    if !env::current_exe()?.starts_with(APP) || agent.exists() {
        return Ok(());
    }
    if let Some(folder) = agent.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(agent, AGENT)
}
