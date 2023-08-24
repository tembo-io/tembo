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
        .arg(
            Arg::new("file-path")
                .short('f')
                .long("file-path")
                .value_name("dir")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .action(ArgAction::Append)
                .help(
                    "A path to the directory to add to the configuration \
                    file to, default is $HOME/.config/tembo",
                ),
        )
}
