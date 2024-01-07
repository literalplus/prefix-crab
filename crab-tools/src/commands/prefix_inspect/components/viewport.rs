use std::ops::{Deref, DerefMut};

use ipnet::Ipv6Net;
use tui_realm_stdlib::states::SpinnerStates;
use tuirealm::{
    command::{Cmd, CmdResult, Direction},
    event::{Key, KeyEvent},
    AttrValue, Attribute, Component, Event, MockComponent, NoUserEvent,
};

use crate::commands::prefix_inspect::{detail::Detail, leaves::Leaves, Msg};

pub enum PerformResult {
    Refresh,
    NextPrefix(Ipv6Net), // pushes self to history
    Status(&'static str),
    ClearStatus,
    Loading,
    Forward,
    None,
}

pub trait ViewportChild {
    fn load(&mut self);
    fn perform(&mut self, cmd: Cmd) -> PerformResult;
}

enum ActiveChild {
    Detail(Detail),
    Leaves(Leaves),
}

impl ActiveChild {
    fn component(&self) -> &dyn MockComponent {
        match self {
            ActiveChild::Detail(it) => &it.component,
            ActiveChild::Leaves(it) => &it.component,
        }
    }

    fn component_mut(&mut self) -> &mut dyn MockComponent {
        match self {
            ActiveChild::Detail(it) => &mut it.component,
            ActiveChild::Leaves(it) => &mut it.component,
        }
    }

    fn prefix(&self) -> Ipv6Net {
        match self {
            ActiveChild::Detail(it) => it.prefix,
            ActiveChild::Leaves(it) => it.prefix,
        }
    }

    fn cycle_mode(&self) -> Self {
        match self {
            Self::Detail(it) => Self::Leaves(Leaves::new(it.prefix)),
            Self::Leaves(it) => Self::Detail(Detail::new(it.prefix)),
        }
    }

    fn clone_with_prefix(&self, prefix: Ipv6Net) -> Self {
        match self {
            Self::Detail(_) => Self::Detail(Detail::new(prefix)),
            Self::Leaves(_) => Self::Leaves(Leaves::new(prefix)),
        }
    }

    fn mode_emoji(&self) -> &'static str {
        match self {
            ActiveChild::Detail(_) => "🕵",
            ActiveChild::Leaves(_) => "🍃",
        }
    }
}

impl Deref for ActiveChild {
    type Target = dyn ViewportChild;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Detail(it) => it,
            Self::Leaves(it) => it,
        }
    }
}

impl DerefMut for ActiveChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Detail(it) => it,
            Self::Leaves(it) => it,
        }
    }
}

const CMD_PREFIX_UP: Cmd = Cmd::Custom("CMD_PREFIX_UP");
const CMD_HISTORY_BACK: Cmd = Cmd::Custom("CMD_HISTORY_BACK");
const CMD_CYCLE_MODE: Cmd = Cmd::Custom("CMD_CYCLE_MODE");
const RES_PREFIX_CHANGED_NAME: &str = "RES_PREFIX_CHANGED";
const RES_PREFIX_CHANGED: CmdResult = CmdResult::Custom(RES_PREFIX_CHANGED_NAME);
const RES_LOADING_NAME: &str = "RES_LOADING";
const RES_LOADING: CmdResult = CmdResult::Custom(RES_LOADING_NAME);

pub struct Viewport {
    active_child: ActiveChild,
    history: Vec<ActiveChild>,
    spinner: SpinnerStates,
}

impl Viewport {
    pub fn new(prefix: &Ipv6Net) -> Self {
        Self {
            active_child: ActiveChild::Detail(Detail::new(*prefix)),
            history: vec![],
            spinner: SpinnerStates {
                sequence: "⣾⣽⣻⢿⡿⣟⣯⣷".chars().collect(),
                ..Default::default()
            },
        }
    }

    fn history_restore(&mut self, state: ActiveChild) -> CmdResult {
        self.active_child = state; // focus flag is retained when pushing to history
        RES_PREFIX_CHANGED
    }

    fn push_details(&mut self, next_prefix: Ipv6Net) -> CmdResult {
        self.swap_active_to_history(ActiveChild::Detail(Detail::new(next_prefix)))
    }

    /// In contrast to push_details(), this retains the mode
    fn push_next_prefix(&mut self, next_prefix: Ipv6Net) -> CmdResult {
        self.swap_active_to_history(self.active_child.clone_with_prefix(next_prefix))
    }

    fn push_mode_cycle(&mut self) -> CmdResult {
        self.swap_active_to_history(self.active_child.cycle_mode())
    }

    fn swap_active_to_history(&mut self, mut new_active: ActiveChild) -> CmdResult {
        new_active
            .component_mut()
            .attr(Attribute::Focus, AttrValue::Flag(true));
        let previous = std::mem::replace(&mut self.active_child, new_active);
        // no really necessary to de-focus the history state, since we only ever use it to set it active again...
        self.history.push(previous);
        self.trigger_load()
    }

    fn trigger_load(&mut self) -> CmdResult {
        self.active_child.load();
        RES_PREFIX_CHANGED
    }
}

impl MockComponent for Viewport {
    fn view(&mut self, frame: &mut tuirealm::Frame, area: tuirealm::tui::prelude::Rect) {
        self.active_child.component_mut().view(frame, area)
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.active_child.component().query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.active_child.component_mut().attr(attr, value)
    }

    fn state(&self) -> tuirealm::State {
        self.active_child.component().state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        use PerformResult as R;

        if let Some(res) = self.perform_internal(cmd) {
            res
        } else {
            match self.active_child.perform(cmd) {
                R::Refresh => self.trigger_load(),
                R::NextPrefix(prefix) => self.push_details(prefix),
                R::Status(line) => CmdResult::Custom(line),
                R::ClearStatus => CmdResult::Custom(""),
                R::Loading => RES_LOADING,
                R::Forward => {
                    self.active_child.component_mut().perform(cmd);
                    CmdResult::Custom("") // signal that view must be updated
                }
                R::None => CmdResult::None,
            }
        }
    }
}

impl Viewport {
    fn perform_internal(&mut self, cmd: Cmd) -> Option<CmdResult> {
        let res = match cmd {
            CMD_PREFIX_UP => {
                if let Some(supernet) = self.active_child.prefix().supernet() {
                    self.push_next_prefix(supernet)
                } else {
                    CmdResult::Custom("Already at the root prefix")
                }
            }
            CMD_HISTORY_BACK => {
                if let Some(state) = self.history.pop() {
                    self.history_restore(state)
                } else {
                    CmdResult::Custom("Already at the root prefix")
                }
            }
            CMD_CYCLE_MODE => self.push_mode_cycle(),
            _ => return None,
        };
        Some(res)
    }
}

impl Component<Msg, NoUserEvent> for Viewport {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<Msg> {
        let cmd = match ev {
            Event::Tick => Cmd::Tick,
            Event::Keyboard(KeyEvent { code, .. }) => match code {
                Key::Char('w') => CMD_PREFIX_UP,
                Key::Backspace => CMD_HISTORY_BACK,
                Key::Enter => Cmd::Submit,
                Key::Up => Cmd::Move(Direction::Up),
                Key::Down => Cmd::Move(Direction::Down),
                Key::PageUp => Cmd::Scroll(Direction::Up),
                Key::PageDown => Cmd::Scroll(Direction::Down),
                Key::Tab => CMD_CYCLE_MODE,
                _ => return None,
            },
            _ => return None,
        };

        match self.perform(cmd) {
            CmdResult::Custom(RES_LOADING_NAME) => {
                Some(Msg::SetStatus(format!("Loading {}", self.spinner.step())))
            }
            CmdResult::Custom(RES_PREFIX_CHANGED_NAME) => Some(Msg::SetStatusPlaceholder(format!(
                // Note that "down" on its own makes little sense as there are two children
                "{} | ⬆⬇ Scroll | ↩ Select{} | w Up | ↹ Mode",
                self.active_child.mode_emoji(),
                if self.history.is_empty() {
                    "".to_string()
                } else {
                    format!(" | ⌫  Back ({})", self.history.len())
                },
            ))),
            CmdResult::Custom(msg) => Some(Msg::SetStatus(msg.to_string())),
            _ => None,
        }
    }
}
