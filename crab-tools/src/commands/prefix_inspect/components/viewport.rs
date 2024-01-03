use std::ops::{Deref, DerefMut};

use ipnet::Ipv6Net;
use tui_realm_stdlib::states::SpinnerStates;
use tuirealm::{
    command::{Cmd, CmdResult, Direction},
    event::{Key, KeyEvent},
    props::Alignment,
    AttrValue, Attribute, Component, Event, MockComponent, NoUserEvent,
};

use crate::commands::prefix_inspect::{detail::Detail, leaves::Leaves, Msg};

pub enum PerformResult {
    Refresh,
    ShowPrefix(Ipv6Net),
    Status(&'static str),
    ClearStatus,
    Loading,
    Forward,
    None,
}

pub trait ViewportChild {
    //fn with_component<T>(&mut self, op: (&mut dyn MockComponent) -> T) -> T;
    //fn component(&self) -> Rc<RefCell<dyn MockComponent>>;
    fn load_for_prefix(&mut self, prefix: Ipv6Net);
    fn perform(&mut self, cmd: Cmd, prefix: Ipv6Net) -> PerformResult;
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

    fn next(&self) -> Self {
        match self {
            Self::Detail(_) => Self::Leaves(Leaves::new()),
            Self::Leaves(_) => Self::Detail(Detail::new()),
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
const CMD_CYCLE_MODE: Cmd = Cmd::Custom("CMD_CYCLE_MODE");
const RES_PREFIX_CHANGED_NAME: &'static str = "RES_PREFIX_CHANGED";
const RES_PREFIX_CHANGED: CmdResult = CmdResult::Custom(RES_PREFIX_CHANGED_NAME);
const RES_LOADING_NAME: &'static str = "RES_LOADING";
const RES_LOADING: CmdResult = CmdResult::Custom(RES_LOADING_NAME);

pub struct Viewport {
    active_child: ActiveChild,
    current_prefix: Ipv6Net,
    spinner: SpinnerStates,
}

impl Viewport {
    fn select_prefix(&mut self, prefix: Ipv6Net) -> CmdResult {
        self.current_prefix = prefix;
        self.active_child.load_for_prefix(prefix);
        self.active_child.component_mut().attr(
            Attribute::Title,
            AttrValue::Title((format!("{:?}", prefix), Alignment::Center)),
        );
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
            match self.active_child.perform(cmd, self.current_prefix) {
                R::Refresh => self.select_prefix(self.current_prefix),
                R::ShowPrefix(prefix) => {
                    self.set_active(ActiveChild::Detail(Detail::new()), prefix)
                }
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
        match cmd {
            CMD_PREFIX_UP => {
                if let Some(supernet) = self.current_prefix.supernet() {
                    Some(self.select_prefix(supernet))
                } else {
                    return Some(CmdResult::Custom("Already at the root prefix"));
                }
            }
            CMD_CYCLE_MODE => Some(self.set_active(self.active_child.next(), self.current_prefix)),
            _ => None,
        }
    }

    fn set_active(&mut self, value: ActiveChild, prefix: Ipv6Net) -> CmdResult {
        self.active_child = value;
        self.active_child
            .component_mut()
            .attr(Attribute::Focus, AttrValue::Flag(true));
        self.select_prefix(prefix)
    }
}

impl Viewport {
    pub fn new(prefix: &Ipv6Net) -> Self {
        let mut spinner = SpinnerStates::default();
        spinner.sequence = "⣾⣽⣻⢿⡿⣟⣯⣷".chars().collect();
        Self {
            active_child: ActiveChild::Detail(Detail::new()),
            current_prefix: *prefix,
            spinner,
        }
    }
}

impl Component<Msg, NoUserEvent> for Viewport {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<Msg> {
        let cmd = match ev {
            Event::Tick => Cmd::Tick,
            Event::Keyboard(KeyEvent { code, .. }) => match code {
                Key::Backspace => CMD_PREFIX_UP,
                Key::Enter => Cmd::Submit,
                Key::Up => Cmd::Move(Direction::Up),
                Key::Down => Cmd::Move(Direction::Down),
                Key::PageUp => Cmd::Scroll(Direction::Down),
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
                "{} | ⬆⬇ Scroll | ↩ Select | ⌫  Up | ↹ Mode",
                self.active_child.mode_emoji()
            ))),
            CmdResult::Custom(msg) => Some(Msg::SetStatus(msg.to_string())),
            _ => None,
        }
    }
}
