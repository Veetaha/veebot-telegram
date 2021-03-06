mod build;
mod clean_data;
mod start;
mod stop;

pub(crate) use build::*;
pub(crate) use clean_data::*;
pub(crate) use start::*;
pub(crate) use stop::*;

pub(crate) trait Cmd {
    fn run(self) -> anyhow::Result<()>;
}
