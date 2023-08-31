use clap::{Arg, ArgAction, Command};

pub fn make_subcommand() -> Command {
    // install is an alias for stack create
    Command::new("install")
        .about("Installs a local Tembo flavored version of Postgres")
        .arg(
            Arg::new("stack")
                .short('s')
                .long("stack")
                .value_name("stack")
                .action(ArgAction::Append)
                .help("The name of a Tembo stack type to install"),
        )
}
