use std::{
    sync::{Arc, Mutex},
    thread,
};

use ipnet::Ipv6Net;
use itertools::Itertools;
use tui_realm_stdlib::{states::SpinnerStates, Textarea};
use tuirealm::{
    command::{Cmd, CmdResult, Direction},
    event::{Key, KeyEvent},
    props::{Alignment, BorderType, Borders, Color, PropPayload, PropValue, TextSpan},
    AttrValue, Attribute, Component, Event, MockComponent, NoUserEvent,
};

use crate::commands::prefix_inspect::Msg;

pub struct Viewport {
    component: Textarea,
    current_prefix: Ipv6Net,
    state: Arc<Mutex<ViewportState>>,
    spinner: SpinnerStates,
}

#[derive(Clone)]
enum ViewportState {
    Missing,
    Loading,
    Ready(String),
    Loaded,
}

impl MockComponent for Viewport {
    fn view(&mut self, frame: &mut tuirealm::Frame, area: tuirealm::tui::prelude::Rect) {
        self.component.view(frame, area)
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.component.query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.component.attr(attr, value)
    }

    fn state(&self) -> tuirealm::State {
        self.component.state()
    }

    fn perform(&mut self, cmd: Cmd) -> tuirealm::command::CmdResult {
        self.current_prefix = match cmd {
            Cmd::Move(Direction::Left) => {
                let mut iter = self
                    .current_prefix
                    .subnets(self.current_prefix.prefix_len() + 1)
                    .expect("to not be max prefix");
                iter.next().unwrap()
            }
            Cmd::Move(Direction::Right) => {
                let mut iter = self
                    .current_prefix
                    .subnets(self.current_prefix.prefix_len() + 1)
                    .expect("to not be max prefix");
                iter.next().unwrap();
                iter.next().unwrap()
            }
            Cmd::Delete => self
                .current_prefix
                .supernet()
                .expect("to not be root prefix"),
            Cmd::Tick => {
                let locked = self.state.lock().expect("not poisoned");
                let copy = locked.clone();
                drop(locked);

                return match copy {
                    ViewportState::Loading => CmdResult::Custom("SPIN"),
                    ViewportState::Missing => {
                        self.trigger_recompute();
                        CmdResult::Custom("Loading!")
                    }
                    ViewportState::Ready(ref res) => {
                        self.display_result(res.to_string());
                        let mut locked = self.state.lock().expect("still not poisoned");
                        *locked = ViewportState::Loaded;
                        CmdResult::Custom("")
                    }
                    _ => CmdResult::None,
                };
            }
            _ => {
                self.component.perform(cmd);
                return CmdResult::Changed(self.state());
            }
        };
        self.trigger_recompute();
        CmdResult::None
    }
}

impl Viewport {
    fn trigger_recompute(&mut self) {
        let mut locked = self.state.lock().expect("mutex poisoned");
        if matches!(*locked, ViewportState::Loading) {
            return;
        }
        *locked = ViewportState::Loading;
        drop(locked);

        self.component.attr(
            Attribute::Title,
            AttrValue::Title((format!("{:?}", self.current_prefix), Alignment::Center)),
        );

        let prefix = self.current_prefix;
        let mutex_ref = Arc::clone(&self.state);
        thread::spawn(move || {
            let res = match super::super::print_prefix(&prefix) {
                Ok(good) => good,
                Err(e) => format!("Error printing prefix: {:?}", e),
            };
            let mut locked = (*mutex_ref).lock().expect("state mutex poisoned");
            *locked = ViewportState::Ready(res);
        });
    }

    fn display_result(&mut self, res: String) {
        let lines = res
            .lines()
            .map(|line| PropValue::TextSpan(TextSpan::from(line)))
            .collect_vec();

        self.component
            .attr(Attribute::Text, AttrValue::Payload(PropPayload::Vec(lines)));
    }

    pub fn new(prefix: &Ipv6Net) -> Self {
        let mut spinner = SpinnerStates::default();
        spinner.sequence = "⣾⣽⣻⢿⡿⣟⣯⣷".chars().collect();
        Self {
            component: Textarea::default()
                .borders(
                    Borders::default()
                        .modifiers(BorderType::Thick)
                        .color(Color::Yellow),
                )
                .highlighted_str("🥕"),
            current_prefix: *prefix,
            state: Mutex::new(ViewportState::Missing).into(),
            spinner,
        }
    }
}

impl Component<Msg, NoUserEvent> for Viewport {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<Msg> {
        return match ev {
            Event::Tick => match self.perform(Cmd::Tick) {
                CmdResult::Custom("SPIN") => {
                    Some(Msg::SetStatus(format!("Loading {}", self.spinner.step())))
                }
                CmdResult::Custom(msg) => Some(Msg::SetStatus(msg.to_string())),
                _ => None,
            },
            Event::Keyboard(KeyEvent {
                code: Key::Home, ..
            }) => {
                if self.current_prefix.supernet().is_some() {
                    self.perform(Cmd::Delete);
                    None
                } else {
                    Some(Msg::SetStatus("Already at root".to_string()))
                }
            }
            Event::Keyboard(KeyEvent {
                code: Key::Left, ..
            }) => {
                if self.current_prefix.prefix_len() >= 64 {
                    Some(Msg::SetStatus("Already at /64".to_string()))
                } else {
                    self.perform(Cmd::Move(Direction::Left));
                    Some(Msg::SetStatus("Moved left".to_string()))
                }
            }
            Event::Keyboard(KeyEvent {
                code: Key::Right, ..
            }) => {
                if self.current_prefix.prefix_len() >= 64 {
                    Some(Msg::SetStatus("Already at /64".to_string()))
                } else {
                    self.perform(Cmd::Move(Direction::Right));
                    Some(Msg::SetStatus("Moved right".to_string()))
                }
            }
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                self.perform(Cmd::Move(Direction::Up));
                Some(Msg::JustRedraw)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => {
                self.perform(Cmd::Move(Direction::Down));
                Some(Msg::JustRedraw)
            }
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => Some(Msg::AppClose),
            Event::Keyboard(KeyEvent {
                code: Key::PageDown,
                ..
            }) => {
                self.perform(Cmd::Scroll(Direction::Down));
                Some(Msg::JustRedraw)
            }
            Event::Keyboard(KeyEvent {
                code: Key::PageUp, ..
            }) => {
                self.perform(Cmd::Scroll(Direction::Up));
                Some(Msg::JustRedraw)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::AppClose),
            _ => None,
        };
    }
}
