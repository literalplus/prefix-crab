use anyhow::Result;
use clap::Args;
use db_model::persist;

use ipnet::Ipv6Net;
use tuirealm::{AttrValue, Attribute, PollStrategy, Update};

use self::app::Model;

mod app;
pub mod detail;
pub mod leaves;
mod components;

#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    persist: persist::Params,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {
    persist::initialize(&params.persist)?;

    println!("Starting...");
    let mut model = Model::new(params.target_prefix)?;
    model.terminal.enter_alternate_screen()?;
    if let Err(e) = model.terminal.enable_raw_mode() {
        model.terminal.leave_alternate_screen()?;
        Err(e)?;
    }

    let res = do_run(&mut model);

    let _ = model.terminal.disable_raw_mode();
    let _ = model.terminal.leave_alternate_screen();

    res
}

fn do_run(model: &mut Model) -> Result<()> {
    while !model.quit {
        match model.app.tick(PollStrategy::Once) {
            Err(err) => {
                model
                    .app
                    .attr(
                        &Id::StatusBar,
                        Attribute::Text,
                        AttrValue::String(format!("Application error: {}", err)),
                    )
                    .unwrap();
            }
            Result::Ok(messages) if !messages.is_empty() => {
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
            model.view()?;
            model.redraw = false;
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    AppClose,
    SetStatus(String),
    SetStatusPlaceholder(String),
    CopyText(String),
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    Viewport,
    StatusBar,
}
