# Tembo CLI

Tembo CLI allows users to experience [Tembo](https://tembo.io) locally, as well as,
manage and deploy to Tembo Cloud. It abstracts away complexities of configuring,
managing, and running Postgres.

## Getting Started

### Installing CLI

Using homebrew

```
brew tap tembo-io/tembo
brew install tembo-cli
```

Using cargo

```
cargo install tembo-cli
```

### Commands

#### `tembo init`

The `tembo init` command initializes your environment with following files. Run init in the directory you want to create the `tembo.toml` file.

* `tembo.toml` example configuration file
* `migrations` directory for sql migrations
* `~/.tembo/context` file with various contexts user can connect to
* `~/.tembo/credentials` file with credentials & api urls

For more information: `tembo init --help`

#### Add Tembo Cloud info

To provision instances on Tembo Cloud using CLI you will need to configure `org_id` & `tembo_access_token`

* fetch the `org_id` from Tembo Cloud and add it as `org_id` in context file generated above
* generate a JWT token using steps [here](https://tembo.io/docs/tembo-cloud/security-and-authentication/api-authentication/) & add it as `tembo_access_token` to the credentials file generated above.

#### `tembo context list/set`

tembo context works like [kubectl context](https://www.notion.so/abee0b15119343e4947692feb740e892?pvs=21). User can set context for local docker environment or tembo cloud (dev/qa/prod) with org_id. When they run any of the other commands it will run in the context selected. Default context will be local.

#### `tembo validate`

Validates `tembo.toml` and other configurations files.

#### `tembo apply`

Validates tembo.toml (same as `tembo validate`) and applies the changes to the context selected. It applies changes and runs migration for all databases.

##### Environment:

  * ###### Local Docker:
    * runs `docker-compose down` to bring down all existing containers
    * generates `Dockerfile` for each instance & builds a docker image
    * generates `docker-compose` to provision all instances
    * runs `docker-compose up -d` to spin up all instances
    * runs `sqlx migration` against the instances

  * ###### Tembo-Cloud: 
    * Creates/updates instance on tembo-cloud by calling the api against the appropriate environment

##### Flags: 
  * `--merge`: Overlays Tembo.toml by another toml file for a specific context
  *  `--set` : Specifies a single instance setting by assigning a new value

#### `tembo logs`

Retrieves log data from the specified Tembo instances. Depending on your current context, it will fetch logs from either local Docker containers or Tembo Cloud instances.

#### `tembo delete`

- **local docker:** runs `docker-compose down` command to bring down all containers
- **tembo-cloud:** deletes the instance on tembo-cloud by calling the api

## Developing Tembo CLI

### Local Testing

Clone this repo and run:

`cargo install --path .`

If the install path is in your shell path, you can then run `tembo help` and other `tembo` commands.

You can run this command to use the local code for any tembo command during development:

```
alias tembo='cargo run --'
```

### Generating Rust Client from API

[OpenAPI Generator](https://openapi-generator.tech/) tool is used to generate Rust Client.

Install OpenAPI Generator if not already by following steps [here](https://openapi-generator.tech/docs/installation)

#### Data plane API client

Go to `tembodataclient` directory in your terminal.

Delete the contents of the directory first and then run following command to re-generate the rust client code for the API.

```bash
openapi-generator generate -i https://api.data-1.use1.tembo.io/api-docs/openapi.json  -g rust -o . --additional-properties=packageName=tembodataclient
```

* Go to `tembodataclient/src/lib.rs` & add following line at the top to disable clippy for the generated code

```
#![allow(clippy::all)]
```

#### Control plane API client

Go to `temboclient` directory in your terminal.

Delete the contents of the directory first and then run following command to re-generate the rust client code for the API.

```bash
openapi-generator generate -i https://api.tembo.io/api-docs/openapi.json  -g rust -o . --additional-properties=packageName=temboclient
```

* Go to `temboclient/src/lib.rs` & add following line at the top to disable clippy for the generated code

```
#![allow(clippy::all)]
```

* Create `/temboclient/src/models/impls.rs` file & add following code to it:

```
use std::str::FromStr;

use super::{Cpu, Environment, Memory, StackType, Storage};

impl FromStr for Cpu {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Cpu, Self::Err> {
        match input {
            "0.25" => Ok(Cpu::Variant0Period25),
            "0.5" => Ok(Cpu::Variant0Period5),
            "1" => Ok(Cpu::Variant1),
            "2" => Ok(Cpu::Variant2),
            "4" => Ok(Cpu::Variant4),
            "8" => Ok(Cpu::Variant8),
            "16" => Ok(Cpu::Variant16),
            "32" => Ok(Cpu::Variant32),
            _ => Err(()),
        }
    }
}

impl FromStr for Memory {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Memory, Self::Err> {
        match input {
            "1Gi" => Ok(Memory::Variant1Gi),
            "2Gi" => Ok(Memory::Variant2Gi),
            "4Gi" => Ok(Memory::Variant4Gi),
            "8Gi" => Ok(Memory::Variant8Gi),
            "16Gi" => Ok(Memory::Variant16Gi),
            "32Gi" => Ok(Memory::Variant32Gi),
            _ => Err(()),
        }
    }
}

impl FromStr for Environment {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Environment, Self::Err> {
        match input {
            "dev" => Ok(Environment::Dev),
            "test" => Ok(Environment::Test),
            "prod" => Ok(Environment::Prod),
            _ => Err(()),
        }
    }
}

impl FromStr for Storage {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<Storage, Self::Err> {
        match input {
            "10Gi" => Ok(Storage::Variant10Gi),
            "50Gi" => Ok(Storage::Variant50Gi),
            "100Gi" => Ok(Storage::Variant100Gi),
            "200Gi" => Ok(Storage::Variant200Gi),
            "300Gi" => Ok(Storage::Variant300Gi),
            "400Gi" => Ok(Storage::Variant400Gi),
            "500Gi" => Ok(Self::Variant500Gi),
            _ => Err(()),
        }
    }
}

impl FromStr for StackType {
    type Err = ();

    fn from_str(input: &str) -> core::result::Result<StackType, Self::Err> {
        match input {
            "Standard" => Ok(StackType::Standard),
            "MessageQueue" => Ok(StackType::MessageQueue),
            "MachineLearning" => Ok(StackType::MachineLearning),
            "OLAP" => Ok(StackType::Olap),
            "VectorDB" => Ok(StackType::VectorDb),
            "OLTP" => Ok(StackType::Oltp),
            "DataWarehouse" => Ok(StackType::DataWarehouse),
            "Geospatial" => Ok(StackType::Geospatial),
            _ => Err(()),
        }
    }
}
```

* Add following line towards the end of `/temboclient/src/models/mod.rs`

```
pub mod impls;
```

## Contributing

Before you start working on something, it's best to check if there is an existing plan
first. Join our [Slack community](https://join.slack.com/t/trunk-crew/shared_invite/zt-1yiafma92-hFHq2xAN0ukjg_2AsOVvfg) and ask there.

## Semver

Tembo CLI is following [Semantic Versioning 2.0](https://semver.org/).
