mod chart;
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

use crate::engine::{Error, Place, Report};

slint::slint! {
    #[edition = "1.19"]
    export { Panel, Tray } from "../ui/panel.slint";
}

const WIDTH: f64 = 440.0;
const MAX_AGE_SECONDS: i64 = 3600;
// A click on the icon takes the focus away first, so the panel is already closed by then.
const SAME_CLICK: Duration = Duration::from_millis(300);

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

struct App {
    // Only while it is open: a window takes several megabytes.
    panel: RefCell<Option<Panel>>,
    tray: Tray,
    areas: RefCell<Vec<String>>,
    selected: Cell<usize>,
    // The area picker: saved areas that match what was typed, then places found.
    typed: RefCell<String>,
    matches: RefCell<Vec<usize>>,
    found: RefCell<Vec<Place>>,
    searched: RefCell<String>,
    searching: Cell<bool>,
    reports: RefCell<HashMap<String, Saved>>,
    loading: RefCell<Option<String>>,
    problem: RefCell<String>,
    closed_at: Cell<Option<Instant>>,
    had_focus: Cell<bool>,
}

struct Saved {
    report: Report,
    at: i64,
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
    select_backend()?;
    let app = Rc::new(App {
        panel: RefCell::default(),
        tray: Tray::new()?,
        areas: RefCell::default(),
        selected: Cell::new(0),
        typed: RefCell::default(),
        matches: RefCell::default(),
        found: RefCell::default(),
        searched: RefCell::default(),
        searching: Cell::new(false),
        reports: RefCell::default(),
        loading: RefCell::default(),
        problem: RefCell::default(),
        closed_at: Cell::new(None),
        had_focus: Cell::new(false),
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
    panel.on_edited(|text| with_app(|app| app.filter(&text)));
    panel.on_search(|text| with_app(|app| app.search(&text)));
    panel.on_refresh(|| with_app(App::fetch));
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
            let position = LogicalPosition::new(x as f32, y as f32);
            panel.window().set_position(position);
        }
        self.panel.replace(Some(panel));
        // The window exists for macOS only after the event loop has turned.
        Timer::single_shot(Duration::ZERO, || with_app(App::focus));
        self.show_selected();
        self.reload_areas(None);
        Ok(())
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
        self.typed.replace(typed.to_string());
        self.matches
            .replace(view::matching(&self.areas.borrow(), typed));
        self.found.borrow_mut().clear();
        self.searched.borrow_mut().clear();
        self.show_choices();
    }

    fn search(&self, text: &str) {
        if self.searching.get() {
            return;
        }
        self.searching.set(true);
        self.searched.replace(text.to_string());
        self.show_choices();
        let typed = text.to_string();
        in_background(
            move || engine::search(&typed),
            |app, result| {
                app.searching.set(false);
                match result {
                    Ok(places) => {
                        app.found.replace(places);
                        app.problem.take();
                        app.show_selected();
                    }
                    Err(error) => app.report_problem(&error),
                }
            },
        );
    }

    fn choose(&self, index: usize) {
        let saved = self.matches.borrow().get(index).copied();
        if let Some(area) = saved {
            self.select(area);
            return;
        }
        let number = index - self.matches.borrow().len();
        let Some(place) = self.found.borrow().get(number).cloned() else {
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
        let text = self.searched.borrow().clone();
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

    fn reload_areas(&self, wanted: Option<String>) {
        in_background(engine::areas, move |app, result| match result {
            Ok(areas) => {
                let previous = wanted.or_else(|| app.selected_area());
                let index = areas
                    .iter()
                    .position(|area| Some(area) == previous.as_ref());
                app.matches
                    .replace(view::matching(&areas, &app.typed.borrow()));
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
            move || engine::report(&name),
            move |app, result| app.store(&area, result),
        );
    }

    fn store(&self, area: &str, result: Result<Report, Error>) {
        if self.loading.borrow().as_deref() == Some(area) {
            self.loading.replace(None);
        }
        match result {
            Ok(report) => {
                let saved = Saved { report, at: now() };
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
        let Some(area) = self.selected_area() else {
            return;
        };
        panel.set_area(area.as_str().into());

        let loading = self.loading.borrow().as_ref() == Some(&area);
        let reports = self.reports.borrow();
        let updated = match reports.get(&area) {
            Some(saved) => {
                fill(panel, view::build(&saved.report, now()));
                let offset = saved.report.summit.utc_offset_seconds;
                format!("Updated {}", clock::clock(saved.at, offset))
            }
            None => {
                clear(panel);
                String::new()
            }
        };
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
        let matches = self.matches.borrow();
        let choices = view::choices(&areas, &matches, &self.found.borrow());
        panel.set_choices(view::model(choices));
        let selected = matches
            .iter()
            .position(|&area| area == self.selected.get())
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1);
        panel.set_selected(selected);
        panel.set_searching(self.searching.get());
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
