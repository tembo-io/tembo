# Trunk CLI

The [Trunk PostgreSQL Extension Registry](https://pgtrunk.io), henceforth Trunk, offers a companion CLI to facilitate a user-friendly programmatic interface. This toolset lowers the barriers to building, sharing, and using PostgreSQL extensions.

## Installation

The Trunk CLI can be installed using the following command:

`cargo install pg-trunk`

By default, the files are stored (). To confirm its proper installation, invoke the following:

`trunk --version`

## Commands - Brief

The CLI toolkit will abstract away many complexities in extension development and installation by using the following commands:
- `trunk build` - compiles extensions and supports nested dependencies.
- `trunk publish` - publishes an extension to the registry, making it available for discovery and installation.
- `trunk install` - download and install the extension distribution, in whichever environment trunk is run.

On the Horizon:
- `trunk init` - setup your environment to build a new Postrgres extension.
- `trunk test` - facilitate the automated unit and integration testing Postgres extensions.

## Commands - Detailed
### 1. `trunk build`

This command leverages [pgrx](https://github.com/tcdi/pgrx) to help you build compiled Postgres extensions. 

Usage: trunk build [OPTIONS]

Options:
- -p, --path <PATH>                [default: .]
- -o, --output-path <OUTPUT_PATH>  [default: ./.trunk]
- -h, --help                       Print help

### 2. `trunk publish`

This command allows you to publish your newly-minted Postgres extension to the Trunk registry.

Usage: trunk publish [OPTIONS] --version <VERSION> <NAME>

Arguments:
  <NAME>

Options:
-  -v, --version <VERSION>
-  -f, --file <FILE>
-  -d, --description <DESCRIPTION>
-  -D, --documentation <DOCUMENTATION>
-  -H, --homepage <HOMEPAGE>
-  -l, --license <LICENSE>
-  -r, --registry <REGISTRY>            [default: https://registry.pgtrunk.io]
-  -R, --repository <REPOSITORY>
-  -h, --help                           Print help


### 3. `trunk install`

This command allows you to install Postgres extensions from the Trunk registry.

Usage: trunk install [OPTIONS]< --version <VERSION> <NAME>

Arguments:
  <NAME>

Options:
-  -p, --pg-config <PG_CONFIG>
-  -f, --file <FILE>
-  -v, --version <VERSION>
-  -r, --registry <REGISTRY>    [default: https://registry.pgtrunk.io]
-  -h, --help                   Print help

## Use Case Example
Soon to come.


# Community Involvement

## Quick and easy:
- Consider starring the repo :star:
- Join our [Trunk discord channel](https://discord.com/channels/1060568981725003789/1089363774357647370)

## More developed:
- Enagage with the [Trunk discord channel](https://discord.com/channels/1060568981725003789/1089363774357647370) community, we're eager to meet you!
- Consider forking the repo and creating pull requests!

# Further Reading

[Documentation](https://coredb-io.github.io/coredb/)
