use std::{
    sync::{Arc, Mutex},
    thread,
};

use ipnet::Ipv6Net;
use itertools::Itertools;
use tui_realm_stdlib::Textarea;
use tuirealm::{
    command::Cmd,
    props::{Alignment, BorderType, Borders, Color, PropPayload, PropValue, TextSpan},
    AttrValue, Attribute, MockComponent,
};

use crate::commands::prefix_inspect::{
    components::viewport::{PerformResult, ViewportChild},
    detail,
};

use super::model::{self, PrintedPrefix};

pub struct Detail {
    pub component: Textarea,
    pub prefix: Ipv6Net, // should not be changed after init
    state: Arc<Mutex<State>>,
    active: Option<PrintedPrefix>,
}

#[derive(Clone)]
enum State {
    Missing,
    Loading,
    Ready(model::Result),
    Loaded,
}

impl Detail {
    pub fn new(prefix: Ipv6Net) -> Self {
        let component = Textarea::default()
            .borders(
                Borders::default()
                    .modifiers(BorderType::Thick)
                    .color(Color::Yellow),
            )
            .highlighted_str("👉")
            .title(format!("{:?}", prefix), Alignment::Center);
        Self {
            component,
            prefix,
            state: Mutex::new(State::Missing).into(),
            active: None,
        }
    }
}

impl ViewportChild for Detail {
    fn perform(&mut self, cmd: Cmd) -> PerformResult {
        match cmd {
            Cmd::Submit => self.on_submit(),
            Cmd::Tick => self.on_tick(),
            _ => PerformResult::Forward,
        }
    }

    fn load(&mut self) {
        let mut locked = self.state.lock().expect("mutex poisoned");
        if matches!(*locked, State::Loading) {
            return;
        }
        *locked = State::Loading;
        drop(locked);

        let mutex_ref = Arc::clone(&self.state);
        let prefix_clone = self.prefix;
        thread::spawn(move || {
            let res = detail::print_prefix(prefix_clone);
            let mut locked = (*mutex_ref).lock().expect("state mutex poisoned");
            *locked = State::Ready(res);
        });
    }

    fn copy_text(&self) -> String {
        if let Some(ref printed) = self.active {
            printed.lines.join("\n")
        } else {
            "<< missing >>".to_owned()
        }
    }
}

impl Detail {
    fn on_submit(&mut self) -> PerformResult {
        use PerformResult as R;

        let active = match self.active.as_ref() {
            None => return R::Status("Not ready"),
            Some(it) => it,
        };
        match active.find_subnet_from_line_index(self.component.states.list_index) {
            Some(idx) => {
                let mut iter = self
                    .prefix
                    .subnets(self.prefix.prefix_len() + 1)
                    .expect("not to be max prefix");
                if idx == 0 {
                    R::NextPrefix(iter.next().unwrap())
                } else {
                    iter.next().unwrap();
                    R::NextPrefix(iter.next().unwrap())
                }
            }
            _ => R::Status("Please select one of the subnets using the arrow keys"),
        }
    }

    fn on_tick(&mut self) -> PerformResult {
        let locked = self.state.lock().expect("not poisoned");
        let copy = locked.clone();
        drop(locked);

        return match copy {
            State::Loading => PerformResult::Loading,
            State::Missing => PerformResult::Refresh,
            State::Ready(ref res) => {
                self.display_result(res.clone());
                let mut locked = self.state.lock().expect("still not poisoned");
                *locked = State::Loaded;
                PerformResult::ClearStatus
            }
            _ => PerformResult::None,
        };
    }

    fn display_result(&mut self, res: model::Result) {
        let lines = match &res {
            Ok(printed) => printed
                .lines
                .iter()
                .map(|line| PropValue::TextSpan(TextSpan::from(line)))
                .collect_vec(),
            Err(e) => vec![PropValue::TextSpan(
                TextSpan::from(format!("{}", e)).fg(Color::Red),
            )],
        };

        self.component
            .attr(Attribute::Text, AttrValue::Payload(PropPayload::Vec(lines)));

        self.active = res.ok();
    }
}
