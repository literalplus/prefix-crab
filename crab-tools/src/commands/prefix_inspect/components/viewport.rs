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

use crate::commands::prefix_inspect::{
    business::{self, PrintedPrefix},
    Msg,
};

pub struct Viewport {
    component: Textarea,
    current_prefix: Ipv6Net,
    state: Arc<Mutex<ViewportState>>,
    active: Option<PrintedPrefix>,
    spinner: SpinnerStates,
}

#[derive(Clone)]
enum ViewportState {
    Missing,
    Loading,
    Ready(business::Result),
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
            Cmd::Submit => {
                if self.active.is_none() {
                    return CmdResult::Custom("Not ready");
                }
                let it = self.active.as_ref().unwrap();
                match it.find_subnet_from_line_index(self.component.states.list_index) {
                    Some(idx) => {
                        let mut iter = self
                            .current_prefix
                            .subnets(self.current_prefix.prefix_len() + 1)
                            .expect("to not be max prefix");
                        if idx == 0 {
                            iter.next().unwrap()
                        } else {
                            iter.next().unwrap();
                            iter.next().unwrap()
                        }
                    }
                    _ => {
                        return CmdResult::Custom(
                            "Please select one of the subnets using the arrow keys",
                        )
                    }
                }
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
                        self.display_result(res.clone());
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
            let res = business::print_prefix(&prefix);
            let mut locked = (*mutex_ref).lock().expect("state mutex poisoned");
            *locked = ViewportState::Ready(res);
        });
    }

    fn display_result(&mut self, res: business::Result) {
        let lines = match &res {
            Ok(printed) => printed
                .lines
                .iter()
                .map(|line| PropValue::TextSpan(TextSpan::from(line)))
                .collect_vec(),
            Err(e) => vec![PropValue::TextSpan(TextSpan::from(format!("{}", e)).fg(Color::Red))],
        };

        self.component
            .attr(Attribute::Text, AttrValue::Payload(PropPayload::Vec(lines)));

        self.active = res.ok();
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
                .highlighted_str("👉"),
            current_prefix: *prefix,
            state: Mutex::new(ViewportState::Missing).into(),
            active: None,
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
                code: Key::Backspace,
                ..
            }) => {
                if self.current_prefix.supernet().is_some() {
                    self.perform(Cmd::Delete);
                    Some(Msg::ResetStatus)
                } else {
                    Some(Msg::SetStatus("Already at root".to_string()))
                }
            }
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => {
                if self.current_prefix.prefix_len() >= 64 {
                    Some(Msg::SetStatus("Already at /64".to_string()))
                } else {
                    let msg = match self.perform(Cmd::Submit) {
                        CmdResult::Custom(msg) => msg,
                        _ => "Something unexpected happened.",
                    };
                    Some(Msg::SetStatus(msg.to_string()))
                }
            }
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                self.perform(Cmd::Move(Direction::Up));
                Some(Msg::ResetStatus)
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => {
                self.perform(Cmd::Move(Direction::Down));
                Some(Msg::ResetStatus)
            }
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => Some(Msg::AppClose),
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::AppClose),
            _ => None,
        };
    }
}
