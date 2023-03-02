pub mod build;
pub mod install;
pub mod publish;
use clap::ValueEnum;

#[derive(ValueEnum, Clone)]
enum ResourceType {
    Db,
    Dbs,
}

pub trait SubCommand {
    fn execute(&self);
}
