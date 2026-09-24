//! `wisp menubar`: a macOS status item that shows what wisp is doing.
//!
//! A background thread collects a [`Report`] every few seconds and hands it to the `AppKit` event
//! loop, which owns every menu object. All the text comes from [`Report::summary`], so the menu
//! and `wisp status` always agree.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context as _;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS as _};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::model::LlamaClient;
use crate::paths;
use crate::status::{self, Report};

const REFRESH: Duration = Duration::from_secs(2);

enum UserEvent {
    Report(Box<Report>),
    Menu(MenuEvent),
}

/// Runs the menu bar item until the user picks Quit.
///
/// # Errors
///
/// Returns an error if the menu cannot be built or `HOME` is not set.
pub fn run(socket: &Path, model: LlamaClient) -> anyhow::Result<()> {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // No Dock icon, no app switcher entry: this is a status item only.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));
    let refresh = spawn_poller(socket.to_owned(), model, event_loop.create_proxy());

    let items = Items::new();
    let actions = Actions {
        paused: false,
        socket: socket.to_owned(),
        wisp_log: paths::daemon_log(socket),
        llama_log: paths::llama_log()?,
        llama_agent: status::launchd_target(paths::LLAMA_AGENT)?,
        refresh,
    };
    let mut state = MenuState { items, actions, tray: None, shown_recent: Vec::new() };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            // AppKit only accepts status items once the event loop is running.
            Event::NewEvents(StartCause::Init) => {
                if let Err(err) = state.show() {
                    tracing::error!("creating the menu bar item: {err:#}");
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(UserEvent::Report(report)) => state.update(&report),
            Event::UserEvent(UserEvent::Menu(event)) => {
                if event.id == *state.items.quit.id() {
                    *control_flow = ControlFlow::Exit;
                } else {
                    state.actions.handle(&event, &state.items);
                }
            }
            _ => {}
        }
    })
}

/// Collects a report every [`REFRESH`], or right away when poked through the returned sender.
fn spawn_poller(
    socket: PathBuf,
    model: LlamaClient,
    proxy: EventLoopProxy<UserEvent>,
) -> mpsc::Sender<()> {
    let (poke, poked) = mpsc::channel();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(err) => {
                tracing::error!("starting the status poller: {err}");
                return;
            }
        };
        loop {
            let report = runtime.block_on(status::collect(&socket, &model));
            if proxy.send_event(UserEvent::Report(Box::new(report))).is_err() {
                return;
            }
            match poked.recv_timeout(REFRESH) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });
    poke
}

struct Items {
    llama: MenuItem,
    daemon: MenuItem,
    recent: Submenu,
    pause: MenuItem,
    restart: MenuItem,
    wisp_log: MenuItem,
    llama_log: MenuItem,
    quit: MenuItem,
}

impl Items {
    fn new() -> Self {
        Self {
            llama: MenuItem::new("llama-server: checking…", false, None),
            daemon: MenuItem::new("daemon: checking…", false, None),
            recent: Submenu::new("Recent suggestions", true),
            pause: MenuItem::new("Pause suggestions", true, None),
            restart: MenuItem::new("Restart llama-server", true, None),
            wisp_log: MenuItem::new("Open wisp log", true, None),
            llama_log: MenuItem::new("Open llama-server log", true, None),
            quit: MenuItem::new("Quit wisp menu", true, None),
        }
    }

    fn menu(&self) -> anyhow::Result<Menu> {
        let menu = Menu::new();
        menu.append_items(&[
            &self.llama,
            &self.daemon,
            &PredefinedMenuItem::separator(),
            &self.recent,
            &PredefinedMenuItem::separator(),
            &self.pause,
            &self.restart,
            &self.wisp_log,
            &self.llama_log,
            &PredefinedMenuItem::separator(),
            &self.quit,
        ])
        .context("building the menu")?;
        Ok(menu)
    }
}

struct MenuState {
    items: Items,
    actions: Actions,
    tray: Option<TrayIcon>,
    shown_recent: Vec<String>,
}

impl MenuState {
    fn show(&mut self) -> anyhow::Result<()> {
        let tray = TrayIconBuilder::new()
            .with_title("wisp")
            .with_menu(Box::new(self.items.menu()?))
            .build()
            .context("creating the status item")?;
        tracing::info!("menu bar item created");
        self.tray = Some(tray);
        Ok(())
    }

    fn update(&mut self, report: &Report) {
        let summary = report.summary();
        if let Some(tray) = &self.tray {
            tray.set_title(Some(&summary.title));
        }
        self.items.llama.set_text(&summary.llama);
        self.items.daemon.set_text(&summary.daemon);
        self.actions.paused = report.paused;
        self.items.pause.set_text(if report.paused {
            "Resume suggestions"
        } else {
            "Pause suggestions"
        });
        // Rebuilding an open submenu makes it flicker, so only touch it when the lines change.
        if summary.recent != self.shown_recent {
            self.replace_recent(&summary.recent);
            self.shown_recent = summary.recent;
        }
    }

    fn replace_recent(&self, lines: &[String]) {
        while self.items.recent.remove_at(0).is_some() {}
        let placeholder = ["(none yet)".to_owned()];
        let lines = if lines.is_empty() { &placeholder[..] } else { lines };
        for line in lines {
            if let Err(err) = self.items.recent.append(&MenuItem::new(line, false, None)) {
                tracing::warn!("adding a recent suggestion to the menu: {err}");
            }
        }
    }
}

struct Actions {
    paused: bool,
    socket: PathBuf,
    wisp_log: PathBuf,
    llama_log: PathBuf,
    llama_agent: String,
    refresh: mpsc::Sender<()>,
}

impl Actions {
    fn handle(&self, event: &MenuEvent, items: &Items) {
        let id = &event.id;
        if id == items.pause.id() {
            if let Err(err) = paths::set_paused(&self.socket, !self.paused) {
                tracing::error!("toggling pause: {err:#}");
            }
            let _ = self.refresh.send(());
        } else if id == items.restart.id() {
            run_in_background("launchctl", &["kickstart", "-k", &self.llama_agent]);
            let _ = self.refresh.send(());
        } else if id == items.wisp_log.id() {
            open(&self.wisp_log);
        } else if id == items.llama_log.id() {
            open(&self.llama_log);
        }
    }
}

fn open(path: &Path) {
    run_in_background("open", &[&path.display().to_string()]);
}

/// Runs a short command off the UI thread, logging failures.
fn run_in_background(program: &str, args: &[&str]) {
    let mut command = std::process::Command::new(program);
    command.args(args);
    let description = format!("{program} {}", args.join(" "));
    std::thread::spawn(move || match command.status() {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!("{description} exited with {status}"),
        Err(err) => tracing::warn!("running {description}: {err}"),
    });
}
