pub mod get;
pub mod create;

pub trait SubCommand {
    fn execute(&self);
}
