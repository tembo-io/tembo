# Motivation

In the course of building [CoreDB](https://coredb.io)'s Managed Postgres SaaS product, we desire a registry of Postgres extensions, and a uniform interface to install them, to exist. To this end, we’re launching an open-source home for Postgres extensions: `trunk`

# Introducing Trunk

Trunk serves as registry where users can publish, search, and download community-made Postgres extensions. Inspired by popular developer hubs, such as [crates.io](http://crates.io) (Rust), [pypi.org](http://pypi.org) (Python), and [npmjs.com](http://npmjs.com) (JavaScript), Trunk aims to foster an information-rich environment. Here, developers can be proud to showcase their contributions, and users can gain insights into valuable metrics on downloads and trends. We also aim to provide an intuitive experience, supported by tutorials and guides.

# Roadmap

`trunk build`

- this command will create compiled binaries for different operating systems
- operating system / CPU architecture / postgres version / extension version
- example: linux / x86 / postgres15.2 / myextension0.1

`trunk publish`

- this command will push compiled binaries to an open source repository

`trnk.io`

- trnk.io is the planned open source repository, available to all postgres extension developers

`trunk install`

- Download and install the extension distribution, in whichever environment trunk is run

`trunk init`
- setup your environment to build a new Postrgres extension.

`trunk test`
- facilitate the automated unit and integration testing Postgres extensions.
