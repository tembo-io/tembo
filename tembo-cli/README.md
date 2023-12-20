# Tembo CLI

Tembo CLI allows users to experience [Tembo](https://tembo.io) locally, as well as,
manage and deploy to Tembo Cloud. It abstracts away complexities of configuring,
managing, and running Postgres in a local environment.

# Local Testing

Clone this repo and run:

`cargo install --path .`

If the install path is in your shell path, you can then run `tembo help` and other `tembo` commands.

You can run this command to use the local code for any tembo command during development:

```
alias tembo='cargo run --'
```

# Commands

## `tembo init`

The `tembo init` command initializes your environment with following files:

* `tembo.toml` configuration file
* `migrations` directory for sql migrations
* `~/.tembo/context` file with various contexts user can connect to

For more information: `tembo init --help`

## `tembo context list/set`

tembo context works like [kubectl context](https://www.notion.so/abee0b15119343e4947692feb740e892?pvs=21). User can set context for local docker environment or tembo cloud (dev/qa/prod) with org_id. When they run any of the other commands it will run in the context selected. Default context will be local.

## `tembo apply`

Validates Tembo.toml (same as `tembo validate`) and applies the changes to the context selected.

* applies changes and runs migration for all dbs
    * **local docker:** wraps docker build/run + sqlx migration
    * **tembo-cloud:** calls the api in appropriate environment

## `tembo delete`

- **local docker:** runs `docker stop & rm` command
- **tembo-cloud:** calls delete tembo api endpoint

## Generating Rust Client from API

[OpenAPI Generator](https://openapi-generator.tech/) tool is used to generate Rust Client.

Install OpenAPI Generator if not already by following steps [here](https://openapi-generator.tech/docs/installation)

### Control plane API client

Go to `temboclient` directory in your terminal.

Delete the contents of the directory first and then run following command to re-generate the rust client code for the API.

```bash
openapi-generator generate -i https://api.tembo.io/api-docs/openapi.json  -g rust -o . --additional-properties=packageName=temboclient
```

* Go to `temboclient/src/lib.rs` & add followng line at the top to disable clippy for the generated code

```
#![allow(clippy::all)]
```

* Create `/temboclient/src/models/impls.rs` file & add following code to it:

**TODO:** Find a better way to do this.

```

use std::str::FromStr;

use super::{Cpu, Storage, StackType, Memory, Environment};

impl FromStr for Cpu {
	type Err = ();

	fn from_str(input: &str) -> core::result::Result<Cpu, Self::Err> {
			match input {
					"1"  => Ok(Cpu::Variant1),
					"2"  => Ok(Cpu::Variant2),
					"4"  => Ok(Cpu::Variant4),
					"8" => Ok(Cpu::Variant8),
					"16" => Ok(Cpu::Variant16),
					"32" => Ok(Cpu::Variant32),
					_      => Err(()),
			}
	}
}

impl FromStr for Memory {
	type Err = ();

	fn from_str(input: &str) -> core::result::Result<Memory, Self::Err> {
			match input {
					"1Gi"  => Ok(Memory::Variant1Gi),
					"2Gi"  => Ok(Memory::Variant2Gi),
					"4Gi"  => Ok(Memory::Variant4Gi),
					"8Gi" => Ok(Memory::Variant8Gi),
					"16Gi" => Ok(Memory::Variant16Gi),
					"32Gi" => Ok(Memory::Variant32Gi),
					_      => Err(()),
			}
	}
}

impl FromStr for Environment {
	type Err = ();

	fn from_str(input: &str) -> core::result::Result<Environment, Self::Err> {
			match input {
					"dev"  => Ok(Environment::Dev),
					"test"  => Ok(Environment::Test),
					"prod"  => Ok(Environment::Prod),
					_      => Err(()),
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

impl ToString for StackType {
	fn to_string(&self) -> String {
			match self {
					Self::Standard => String::from("Standard"),
					Self::MessageQueue => String::from("MessageQueue"),
					Self::MachineLearning => String::from("MachineLearning"),
					Self::Olap => String::from("OLAP"),
					Self::Oltp => String::from("OLTP"),
					Self::VectorDb => String::from("VectorDB"),
					Self::DataWarehouse => String::from("DataWarehouse"),
			}
	}
}
```

* Add following line towards the end of `/temboclient/src/models/mod.rs`

```
pub mod impls;
```

# Contributing

Before you start working on something, it's best to check if there is an existing plan
first. Join our [Slack community](https://join.slack.com/t/trunk-crew/shared_invite/zt-1yiafma92-hFHq2xAN0ukjg_2AsOVvfg) and ask there.

# Semver

Tembo CLI is following [Semantic Versioning 2.0](https://semver.org/).
