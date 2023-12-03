use anyhow::*;
use clap::Args;
use db_model::{persist, prefix_tree::PrefixTree};
use futures::executor;
use ipnet::Ipv6Net;
use log::info;

use prefix_crab::helpers::rabbit::RabbitHandle;
use queue_models::probe_request::EchoProbeRequest;
use queue_models::RoutedMessage;
use tuirealm::{AttrValue, Attribute, PollStrategy, Update};

use crate::rabbit;

use self::app::Model;

mod app;
mod components;

#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    persist: persist::Params,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {
    let mut model = Model::default();
    let _ = model.terminal.enter_alternate_screen();
    let _ = model.terminal.enable_raw_mode();

    while !model.quit {
        match model.app.tick(PollStrategy::Once) {
            Err(err) => {
                assert!(model
                    .app
                    .attr(
                        &Id::StatusBar,
                        Attribute::Text,
                        AttrValue::String(format!("Application error: {}", err)),
                    )
                    .is_ok());
            }
            Result::Ok(messages) if messages.len() > 0 => {
                model.redraw = true;
                for msg in messages.into_iter() {
                    let mut msg = Some(msg);
                    while msg.is_some() {
                        msg = model.update(msg);
                    }
                }
            }
            _ => {}
        }

        if model.redraw {
            model.view();
            model.redraw = false;
        }
    }

    let _ = model.terminal.leave_alternate_screen();
    let _ = model.terminal.disable_raw_mode();
    let _ = model.terminal.clear_screen();
    Ok(())
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    AppClose,
    BaseDataLoaded,
    LhrsLoaded,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    BaseInfo,
    Lhrs,
    StatusBar,
}


pub enum AppEvent {
    BaseDataLoaded(PrefixTree),
}