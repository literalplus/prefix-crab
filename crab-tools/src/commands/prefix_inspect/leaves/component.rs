use std::{
    sync::{Arc, Mutex},
    thread,
};

use ipnet::Ipv6Net;
use itertools::Itertools;
use tui_realm_stdlib::Table;
use tuirealm::{
    command::Cmd,
    props::{BorderType, Borders, Color, TableBuilder, TextSpan},
    AttrValue, Attribute, MockComponent,
};

use crate::commands::prefix_inspect::components::viewport::{PerformResult, ViewportChild};

use super::model::{self, LeafNet};

pub struct Leaves {
    pub component: Table,
    state: Arc<Mutex<State>>,
    active: Option<Vec<LeafNet>>,
}

#[derive(Clone)]
enum State {
    Missing,
    Loading,
    Ready(model::Result),
    Loaded,
}

const NET_ROW_WIDTH: u16 = 50;

impl Leaves {
    pub fn new() -> Self {
        let component = Table::default()
            .borders(
                Borders::default()
                    .modifiers(BorderType::Thick)
                    .color(Color::Yellow),
            )
            .highlighted_color(Color::DarkGray)
            .scroll(true)
            .headers(&["🎋", "💰", "💪"])
            .widths(&[NET_ROW_WIDTH, 20, 15]);
        Self {
            component,
            state: Mutex::new(State::Missing).into(),
            active: None,
        }
    }
}

impl ViewportChild for Leaves {
    fn perform(&mut self, cmd: Cmd, prefix: Ipv6Net) -> PerformResult {
        match cmd {
            Cmd::Submit => self.on_submit(),
            Cmd::Tick => self.on_tick(prefix.prefix_len()),
            _ => return PerformResult::Forward,
        }
    }

    fn load_for_prefix(&mut self, prefix: Ipv6Net) {
        let mut locked = self.state.lock().expect("mutex poisoned");
        if matches!(*locked, State::Loading) {
            return;
        }
        *locked = State::Loading;
        drop(locked);

        let mutex_ref = Arc::clone(&self.state);
        thread::spawn(move || {
            let res = super::find_leaves(&prefix);
            let mut locked = (*mutex_ref).lock().expect("state mutex poisoned");
            *locked = State::Ready(res);
        });
    }
}

impl Leaves {
    fn on_submit(&mut self) -> PerformResult {
        use PerformResult as R;

        let active = match self.active.as_ref() {
            None => return R::Status("Not ready"),
            Some(it) => it,
        };
        let index = self.component.states.list_index;
        R::ShowPrefix(active[index].net)
    }

    fn on_tick(&mut self, own_prefix_len: u8) -> PerformResult {
        let locked = self.state.lock().expect("not poisoned");
        let copy = locked.clone();
        drop(locked);

        return match copy {
            State::Loading => PerformResult::Loading,
            State::Missing => PerformResult::Refresh,
            State::Ready(ref res) => {
                self.display_result(res.clone(), own_prefix_len);
                let mut locked = self.state.lock().expect("still not poisoned");
                *locked = State::Loaded;
                PerformResult::ClearStatus
            }
            _ => PerformResult::None,
        };
    }

    fn display_result(&mut self, res: model::Result, own_prefix_len: u8) {
        let table = match &res {
            Ok(nets) => nets
                .into_iter()
                .map(|net| net_to_row(net, own_prefix_len))
                .collect_vec(),
            Err(e) => TableBuilder::default()
                .add_col(TextSpan::from(format!("{}", e)).fg(Color::Red))
                .add_col(TextSpan::from("❌"))
                .add_col(TextSpan::from("❌"))
                .build(),
        };

        self.component
            .attr(Attribute::Content, AttrValue::Table(table));

        self.active = res.ok();
    }
}

fn net_to_row(net: &LeafNet, own_prefix_len: u8) -> Vec<TextSpan> {
    let net_format_len = format!("{}", net.net).len();
    let net_format_len = net_format_len.try_into().unwrap_or(u16::MAX);
    let available_space = NET_ROW_WIDTH.saturating_sub(net_format_len);

    let len_diff = own_prefix_len.abs_diff(net.net.prefix_len()) as u16;
    let len_diff = len_diff.clamp(0, available_space);
    let indent = " ".repeat(len_diff as usize);
    vec![
        TextSpan::from(format!("{}{}", indent, net.net)),
        TextSpan::from(format!("{:?}", net.priority_class)),
        TextSpan::from(format!("{}%", net.confidence)),
    ]
}
