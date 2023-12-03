use std::time::Duration;

use ipnet::Ipv6Net;
use tuirealm::event::NoUserEvent;
use tuirealm::props::{Alignment, Color, TextModifiers};
use tuirealm::terminal::TerminalBridge;
use tuirealm::tui::layout::{Constraint, Direction, Layout};
use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Update,
};


use crate::commands::prefix_inspect::components::base_info::BaseInfo;
use crate::commands::prefix_inspect::components::status_bar::StatusBar;

use super::components::lhr_list::LhrList;
use super::{Id, Msg};

pub struct Model {
    pub app: Application<Id, Msg, NoUserEvent>,
    pub quit: bool,
    pub redraw: bool,
    pub terminal: TerminalBridge,
}

impl Model {
    pub fn new(prefix: Ipv6Net) -> Self {
        Self {
            app: Self::init_app(prefix),
            quit: false,
            redraw: true,
            terminal: TerminalBridge::new().expect("Cannot initialize terminal"),
        }
    }

    pub fn view(&mut self) {
        assert!(self
            .terminal
            .raw_mut()
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints(
                        [
                            Constraint::Length(1), // Base info
                            Constraint::Length(6), // Main frame
                            Constraint::Length(1), // Status bar
                        ]
                        .as_ref(),
                    )
                    .split(f.size());
                self.app.view(&Id::BaseInfo, f, chunks[0]);
                self.app.view(&Id::Lhrs, f, chunks[1]);
                self.app.view(&Id::StatusBar, f, chunks[2]);
            })
            .is_ok());
    }

    fn init_app(prefix: Ipv6Net) -> Application<Id, Msg, NoUserEvent> {
        let mut app: Application<Id, Msg, NoUserEvent> = Application::init(
            EventListenerCfg::default()
                .default_input_listener(Duration::from_millis(20))
                .poll_timeout(Duration::from_millis(10))
                .tick_interval(Duration::from_secs(1)),
        );
        // Mount components
        app.mount(
            Id::StatusBar,
            Box::new(
                StatusBar::default()
                    .text("Loading...")
                    .alignment(Alignment::Left)
                    .background(Color::Reset)
                    .foreground(Color::LightYellow)
                    .modifiers(TextModifiers::BOLD),
            ),
            Vec::default(),
        ).expect("mount status bar");
        app.mount(
            Id::BaseInfo,
            Box::new(BaseInfo::loading_placeholder(prefix)),
            Vec::default()
        ).expect("mount base info");
        app.mount(
            Id::Lhrs,
            Box::new(LhrList::new(&vec!["Loading...".to_string()])),
            Vec::new()
        ).expect("mount lhrs");

        app
    }
}

impl Update<Msg> for Model {
    fn update(&mut self, msg: Option<Msg>) -> Option<Msg> {
        if let Some(msg) = msg {
            // Set redraw
            self.redraw = true;
            // Match message
            match msg {
                Msg::AppClose => {
                    self.quit = true; // Terminate
                    None
                }
                Msg::BaseDataLoaded => {
                    assert!(self.app.active(&Id::BaseInfo).is_ok());
                    None
                }
                Msg::LhrsLoaded(v) => {
                    self.app.attr(&Id::Lhrs, Attribute::, value)
                    // Update label
                    assert!(self
                        .app
                        .attr(
                            &Id::Label,
                            Attribute::Text,
                            AttrValue::String(format!("DigitCounter has now value: {}", v))
                        )
                        .is_ok());
                    None
                }
            }
        } else {
            None
        }
    }
}
