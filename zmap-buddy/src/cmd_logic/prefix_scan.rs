#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: super::ZmapBaseParams,

    target_prefix: String,
}

pub fn handle(params: Params) -> anyhow::Result<()> {
    params.base.into_caller()?;
    panic!("not implemented :(");
}
