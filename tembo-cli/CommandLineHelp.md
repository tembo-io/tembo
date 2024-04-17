# Command-Line Help for `tembo-cli`

This document contains the help content for the `tembo-cli` command-line program.

**Command Overview:**

* [`tembo-cli`↴](#tembo-cli)
* [`tembo-cli context`↴](#tembo-cli-context)
* [`tembo-cli context list`↴](#tembo-cli-context-list)
* [`tembo-cli context set`↴](#tembo-cli-context-set)
* [`tembo-cli init`↴](#tembo-cli-init)
* [`tembo-cli apply`↴](#tembo-cli-apply)
* [`tembo-cli validate`↴](#tembo-cli-validate)
* [`tembo-cli delete`↴](#tembo-cli-delete)
* [`tembo-cli logs`↴](#tembo-cli-logs)
* [`tembo-cli login`↴](#tembo-cli-login)
* [`tembo-cli top`↴](#tembo-cli-top)

## `tembo-cli`

Tembo CLI

**Usage:** `tembo-cli [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `context` — Manage Tembo contexts
* `init` — Initializes a local environment. Creates a sample context and configuration files
* `apply` — Deploys a tembo.toml file
* `validate` — Validates the tembo.toml file, context file, etc
* `delete` — Deletes database instance locally or on Tembo Cloud
* `logs` — View logs for your instance
* `login` — Initiates login sequence to authenticate with Tembo
* `top` — [EXPERIMENTAL] View Metric values of your instances

###### **Options:**

* `--markdown-help`
* `-v`, `--verbose` — Show more information in command output



## `tembo-cli context`

Manage Tembo contexts

**Usage:** `tembo-cli context <COMMAND>`

###### **Subcommands:**

* `list` — List all available contexts
* `set` — Set the current context



## `tembo-cli context list`

List all available contexts

**Usage:** `tembo-cli context list`



## `tembo-cli context set`

Set the current context

**Usage:** `tembo-cli context set --name <NAME>`

###### **Options:**

* `-n`, `--name <NAME>`



## `tembo-cli init`

Initializes a local environment. Creates a sample context and configuration files

**Usage:** `tembo-cli init`



## `tembo-cli apply`

Deploys a tembo.toml file

**Usage:** `tembo-cli apply [OPTIONS]`

###### **Options:**

* `-m`, `--merge <MERGE>` — Merge the values of another tembo.toml file to this file before applying
* `-s`, `--set <SET>` — Replace a specific configuration in your tembo.toml file. For example, tembo apply --set standard.cpu = 0.25



## `tembo-cli validate`

Validates the tembo.toml file, context file, etc

**Usage:** `tembo-cli validate`



## `tembo-cli delete`

Deletes database instance locally or on Tembo Cloud

**Usage:** `tembo-cli delete`



## `tembo-cli logs`

View logs for your instance

**Usage:** `tembo-cli logs`



## `tembo-cli login`

Initiates login sequence to authenticate with Tembo

**Usage:** `tembo-cli login [OPTIONS]`

###### **Options:**

* `--organization-id <ORGANIZATION_ID>` — Set your Org ID for your new environment, which starts with "org_"
* `--profile <PROFILE>` — Set a name for your new environment, for example "prod". This name will be used for the name of the environment and the credentials profile
* `--tembo-host <TEMBO_HOST>` — Set your tembo_host for your profile, for example api.tembo.io
* `--tembo-data-host <TEMBO_DATA_HOST>` — Set your tembo_data_host for your profile, for example api.data-1.use1.tembo.io



## `tembo-cli top`

[EXPERIMENTAL] View Metric values of your instances

**Usage:** `tembo-cli top [OPTIONS]`

###### **Options:**

* `--tail`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

