pub mod create;
pub mod get;
pub mod install;
use clap::ValueEnum;

#[derive(ValueEnum, Clone)]
enum ResourceType {
    Db,
    Dbs,
}

pub trait SubCommand {
    fn execute(&self);
}
