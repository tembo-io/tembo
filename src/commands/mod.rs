pub mod build;
pub mod install;
mod pgx;
pub mod publish;

pub trait SubCommand {
    fn execute(&self);
}
