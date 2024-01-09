use std::{
    sync::{Arc, Mutex},
    thread,
};

use db_model::prefix_tree::PriorityClass;
use ipnet::Ipv6Net;
use itertools::Itertools;
use tui_realm_stdlib::Table;
use tuirealm::{
    command::Cmd,
    props::{Alignment, BorderType, Borders, Color, TableBuilder, TextSpan},
    AttrValue, Attribute, MockComponent,
};

use crate::commands::prefix_inspect::components::viewport::{PerformResult, ViewportChild};

use super::model::{self, LeafNet};

pub struct Leaves {
    pub component: Table,
    pub prefix: Ipv6Net, // should not change during lifetime
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
    pub fn new(prefix: Ipv6Net) -> Self {
        let component = Table::default()
            .borders(
                Borders::default()
                    .modifiers(BorderType::Thick)
                    .color(Color::Yellow),
            )
            .highlighted_color(Color::DarkGray)
            .scroll(true)
            .headers(&["🎋", "👣", "💰", "💪"])
            .widths(&[NET_ROW_WIDTH, 5, 20, 4])
            .title(format!("{:?}", prefix), Alignment::Center);
        Self {
            component,
            prefix,
            state: Mutex::new(State::Missing).into(),
            active: None,
        }
    }
}

impl ViewportChild for Leaves {
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
            let res = super::find_leaves(prefix_clone);
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
        R::NextPrefix(active[index].net)
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
        let own_prefix_len = self.prefix.prefix_len();
        let table = match &res {
            Ok(nets) => nets
                .iter()
                .map(|net| net_to_row(net, own_prefix_len))
                .collect_vec(),
            Err(e) => TableBuilder::default()
                .add_col(TextSpan::from(format!("{}", e)).fg(Color::Red))
                .add_col(TextSpan::from("❌"))
                .add_col(TextSpan::from("❌"))
                .add_col(TextSpan::from("❌"))
                .build(),
        };

        self.component
            .attr(Attribute::Content, AttrValue::Table(table));

        let found_nets = res.as_ref().map(|it| it.len()).unwrap_or(0);
        self.component.attr(
            Attribute::Title,
            AttrValue::Title((
                format!("{} ({})", self.prefix, found_nets),
                Alignment::Center,
            )),
        );

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

    let prio_suffix = if net.redundant { " 🍂" } else { "" };

    let confidence = if net.net.prefix_len() >= 64 {
        "--".to_string()
    } else {
        format!("{}%", net.confidence)
    };

    vec![
        TextSpan::from(format!("{}{}", indent, net.net)),
        TextSpan::from(&net.hash_short),
        TextSpan::from(format!("{:?}{}", net.priority_class, prio_suffix))
            .fg(prio_color(net.priority_class)),
        TextSpan::from(confidence),
    ]
}

fn prio_color(prio: PriorityClass) -> Color {
    use PriorityClass as P;

    match prio {
        P::MediumSameRatio => Color::Gray,
        P::MediumSameSingle => Color::DarkGray,
        P::HighDisjoint => Color::Magenta,
        P::HighFresh => Color::LightGreen,
        _ => Color::Reset,
    }
}
