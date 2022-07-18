# MassBit Chain

## Build

> This section assumes that the developer is running on either macOS or Debian-variant operating system. For Windows, although there are ways to run it, we recommend using [WSL](https://docs.microsoft.com/en-us/windows/wsl/install-win10) or from a virtual machine for stability.

Execute the following command from your terminal to set up the development environment and build the node runtime.

```bash
# install Substrate development environment via the automatic script
$ curl https://getsubstrate.io -sSf | bash -s -- --fast

# clone the Git repository
$ git clone https://github.com/massbitprotocol/massbitchain.git

# change current working directory
$ cd massbitchain 

# compile the node
# note: you may encounter some errors if `wasm32-unknown-unknown` is not installed, or if the toolchain channel is outdated
$ cargo build --release

# show list of available commands
$ ./target/release/massbit-node --help
```

## Run
### Development node
This command will start the single-node development chain with non-persistent state:

```bash
./target/release/massbitchain --dev
```

Purge the development chain's state:

```bash
./target/release/massbitchain purge-chain --dev
```

