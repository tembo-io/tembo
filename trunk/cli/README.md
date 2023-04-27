# Trunk CLI

The [Trunk PostgreSQL Extension Registry](https://pgtrunk.io), henceforth Trunk, offers a companion CLI (command line interface) to facilitate a user-friendly programmatic interface. This toolset, written in Rust, leverages the package manager, Cargo, and lowers the barriers to building, sharing, and using PostgreSQL extensions.

## Installation

The Trunk CLI can be installed using the following command:

`cargo install pg-trunk`

By default, the files are stored (). To confirm its proper installation, invoke the following:

`trunk --version`

## Commands

The CLI toolkit will abstract away many complexities in extension development and installation by using the following commands:

- `trunk init` - setup your environment to build a new Postrgres extension.
- `trunk test` - facilitate the automated unit and integration testing Postgres extensions.
- `trunk build` - compiles extensions and supports nested dependencies.
- `trunk publish` - publishes an extension to the registry, making it available for discovery and installation.
- `trunk install` - download and install the extension distribution, in whichever environment trunk is run.

### 1. `trunk init`

### 2. `trunk test`

### 3. `trunk build`
e.g. installing extension_a will automatically install extension_b if required

Usage: trunk build [OPTIONS]

Options:
- -p, --path <PATH>                [default: .]
- -o, --output-path <OUTPUT_PATH>  [default: ./.trunk]
- -h, --help                       Print help

### 4. `trunk publish`

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


### 5. `trunk install`

Usage: trunk install [OPTIONS] --version <VERSION> <NAME>

Arguments:
  <NAME>

Options:
-  -p, --pg-config <PG_CONFIG>
-  -f, --file <FILE>
-  -v, --version <VERSION>
-  -r, --registry <REGISTRY>    [default: https://registry.pgtrunk.io]
-  -h, --help                   Print help

## Use Case Example



# Community Involvement



# Further Reading

[Documentation](https://coredb-io.github.io/coredb/)
