# Tembo CLI

Tembo CLI allows users to experience [Tembo](https://tembo.io) locally, as well as, 
manage and deploy to Tembo Cloud. It abstracts away complexities of configuring, 
managing, and running Postgres in a local environment. 

# Local Testing

Clone this repo and run:

`cargo install --path .`

If the install path is in your shell path, you can then run `tembo help` and other `tembo` commands.

# Commands

## `tembo init`

The `init` command initializes your environment and can be used to generate configuration files. It will
also alert you to any missing requirements. Currently, the only requirement is Docker be running. After 
ensuring the requirements are met, the command will pull the Tembo Docker image.

The default configuration file path is $HOME/.config/tembo.

For more information: `tembo init --help`

## `tembo instance create`

The `instance create` command creates an instance of a Tembo stack locally. It includes the Tembo flavored 
version of Postgres and an additional items like extensions. You can specify the 
type of instance you want to create. You'll also need to provide a name and port number.

Currently supported types include: 

* standard
* data-warehouse

More stack types will be added shortly.

## `tembo instance list`

The `instance list` command simply lists all of the instances that have been created. It lists key attributes such as name, type and port.

## `tembo instance start`

The `instance start` command allows users to start their instances. It requires the name as a parameter and Docker to be running. No two 
instances can be started that share a port number.

Each instance runs as a Docker container.

## `tembo auth login`

The `auth login` command allows users to authenticate as a service user and obtain an API token that can be used on future authenticated requests.

## `tembo auth info`

The `auth info` command allows users to see if they have authenticated and when their authentication token expires.

## `tembo extension install`

The `extension install` command allows users to install extensions on existing instances. Users will be prompted for the 
name and version of the extension. Note this doesn't enable the extension. That is done via the `extension enable` command (WIP).

List of supported extensions can be found on [Trunk](https://pgt.dev).

# Contributing

Before you start working on something, it's best to check if there is an existing plan 
first. Join our [Slack community](https://join.slack.com/t/trunk-crew/shared_invite/zt-1yiafma92-hFHq2xAN0ukjg_2AsOVvfg) and ask there.

# Semver

Tembo CLI is following [Semantic Versioning 2.0](https://semver.org/).
