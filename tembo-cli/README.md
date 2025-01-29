# Tembo CLI

Tembo CLI allows users to experience [Tembo](https://tembo.io) locally, as well as,
manage and deploy to Tembo Cloud. It abstracts away complexities of configuring,
managing, and running Postgres.

## Getting Started

### Installing CLI

Using homebrew

``` sh
brew tap tembo-io/tembo
brew install tembo-cli
```

Using cargo

``` sh
cargo install tembo-cli
```

### Commands
Discover a wide range of commands and subcommands, along with their respective options, by exploring our comprehensive [Command Reference](https://tembo.io/docs/development/cli/command-reference).

## Developing Tembo CLI

### Local Testing

Clone this repo and run:

``` sh
cargo install --path .
```

If the install path is in your shell path, you can then run `tembo help` and other `tembo` commands.

You can run this command to use the local code for any tembo command during development:

``` sh
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

``` rs
#![allow(clippy::all)]
```

#### Control plane API client

Go to `temboclient` directory in your terminal.

Delete the contents of the directory first and then run following command to re-generate the rust client code for the API.

```bash
openapi-generator generate -i https://api.tembo.io/api-docs/openapi.json  -g rust -o . --additional-properties=packageName=temboclient
```

* Go to `temboclient/src/lib.rs` & add following line at the top to disable clippy for the generated code

``` rs
#![allow(clippy::all)]
```

* Create `temboclient/src/models/impls.rs` file & add following code to it:

```rs
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
            "Analytics" => Ok(StackType::Analytics),
            "Geospatial" => Ok(StackType::Geospatial),
            "MachineLearning" => Ok(StackType::MachineLearning),
            "MessageQueue" => Ok(StackType::MessageQueue),
            "MongoAlternative" => Ok(StackType::MongoAlternative),
            "OLTP" => Ok(StackType::OLTP),
            "ParadeDB" => Ok(StackType::ParadeDB),
            "Standard" => Ok(StackType::Standard),
            "Timeseries" => Ok(StackType::Timeseries),
            "VectorDB" => Ok(StackType::VectorDB),
            _ => Err(()),
        }
    }
}
```

* Add following line towards the end of `temboclient/src/models/mod.rs`

``` rs
pub mod impls;
```

## Contributing

Before you start working on something, it's best to check if there is an existing plan
first. Join our [Slack community](https://join.slack.com/t/trunk-crew/shared_invite/zt-1yiafma92-hFHq2xAN0ukjg_2AsOVvfg) and ask there.

## Semver

Tembo CLI is following [Semantic Versioning 2.0](https://semver.org/).
