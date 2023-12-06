use std::time::Duration;

use anyhow::Result;
use ipnet::Ipv6Net;
use tuirealm::event::NoUserEvent;
use tuirealm::props::{Alignment, Color, TextModifiers};
use tuirealm::terminal::TerminalBridge;
use tuirealm::tui::layout::{Constraint, Direction, Layout};
use tuirealm::{Application, AttrValue, Attribute, EventListenerCfg, Update};

use crate::commands::prefix_inspect::components::status_bar::StatusBar;
use crate::commands::prefix_inspect::components::viewport::Viewport;

use super::{Id, Msg};

pub struct Model {
    pub app: Application<Id, Msg, NoUserEvent>,
    pub quit: bool,
    pub redraw: bool,
    pub terminal: TerminalBridge,
}

impl Model {
    pub fn new(prefix: Ipv6Net) -> Result<Self> {
        Ok(Self {
            app: Self::init_app(prefix)?,
            quit: false,
            redraw: true,
            terminal: TerminalBridge::new().expect("Cannot initialize terminal"),
        })
    }

    fn init_app(prefix: Ipv6Net) -> Result<Application<Id, Msg, NoUserEvent>> {
        let mut app: Application<Id, Msg, NoUserEvent> = Application::init(
            EventListenerCfg::default()
                .default_input_listener(Duration::from_millis(20))
                .poll_timeout(Duration::from_millis(10))
                .tick_interval(Duration::from_millis(100)),
        );
        app.mount(
            Id::StatusBar,
            Box::new(
                StatusBar::default()
                    .text("Loading...")
                    .alignment(Alignment::Left)
                    .background(Color::Black)
                    .foreground(Color::White)
                    .modifiers(TextModifiers::BOLD),
            ),
            Vec::default(),
        )?;
        app.mount(
            Id::Viewport,
            Box::new(Viewport::new(&prefix)),
            vec![],
        )?;

        app.active(&Id::Viewport)?;

        Ok(app)
    }

    pub fn view(&mut self) -> Result<()> {
        self.terminal.raw_mut().draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Min(2),    // Base info
                        Constraint::Length(1), // Status bar
                    ]
                    .as_ref(),
                )
                .split(f.size());
            self.app.view(&Id::Viewport, f, chunks[0]);
            self.app.view(&Id::StatusBar, f, chunks[1]);
        })?;
        Ok(())
    }
}

impl Update<Msg> for Model {
    fn update(&mut self, msg: Option<Msg>) -> Option<Msg> {
        if let Some(msg) = msg {
            self.redraw = true;
            match msg {
                Msg::AppClose => {
                    self.quit = true;
                    None
                }
                Msg::SetStatus(status) => {
                    let status = if status.is_empty() {
                        "⬆⬇ Scroll  | ⬅➡ Descend  | 🏠 Ascend  | q Quit".to_string()
                    } else {
                        status
                    };
                    self.app
                        .attr(&Id::StatusBar, Attribute::Text, AttrValue::String(status))
                        .expect("set status bar");
                    None
                }
                Msg::JustRedraw => None,
            }
        } else {
            None
        }
    }
}
