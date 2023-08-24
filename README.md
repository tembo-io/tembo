# Tembo CLI

The Tembo CLI allows users to experience [Tembo](https://tembo.io) locally, as well as, 
manage and deploy to the Tembo cloud. It abstracts away complexities of configuring, 
managing, and running Postgres in a local environment. The Tembo CLI aims to provide the 
best experience working with Postgres in any environment.

# Getting Started

This repo is a work in progress. Soon, it will provide install instructions and a 
detailed Getting Started guide.

# Local Testing
If you would like to test out the CLI locally, you can clone this repo and run:

`cargo install --path .`

If the install path is in your shell path, you can then run:

`tembo init`

or 

`tembo install`

# Commands

`tembo help`

The help command will respond with the various commands and options available.

`tembo init`

The `init` command initializes your environment and can be used to generate configuration files. 
The command supports a `dryrun` flag to test where a configuration file will be written. It also 
supports a `file-path` flag that can be used to explicitly provide an absolute or relative file 
path for the configuration file.

The default configuration file path is $HOME/.config/tembo.

The `init` command can be used to create global and project specific configuration files.

For more information: `tembo init --help`

`tembo install`

The `install` command is an alias for the `stack create` command.

`tembo stack create`

The `stack create` command is used to install a local instance of a Tembo cluser locally. Because it 
is only a single instance, it is called a stack. It includes the Tembo flavored version of Postgres and 
an additional items like extensions. It all runs in a Docker container. That is the only hard requirement.

The valid stack types are: standard and data-warehouse. More stack types will be added shortly.

Next: `tembo start` - which will start an installed stack

# Contributing

Before you start working on something, it's best to check if there is an existing plan 
first. Stop by the Discord server and confirm with the team if it makes sense or if 
someone else is already working on it.

A more detailed Contributing Guide is coming soon.

# Semver

Tembo CLI is following [Semantic Versioning 2.0](https://semver.org/).

