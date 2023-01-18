pub mod create;
pub mod get;

pub trait SubCommand {
    fn execute(&self);
}
