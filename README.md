# Ethers UserOp
An ether-rs middleware to craft UserOperations

## Pre-requisites
[Geth](https://geth.ethereum.org/docs/getting-started/installing-geth) (tested with v1.12.2).

## Use
To start a [Silius](https://github.com/Vid201/silius) bundler with user operation pool and JSON-RPC API with default config at `127.0.0.1:3000` on [Geth Testnet](https://chainlist.org/chain/1337)
```bash
cargo run --bin bundler
```
To create a [SimpleAccount smart contract wallet](https://github.com/eth-infinitism/account-abstraction/blob/develop/contracts/samples/SimpleAccount.sol)
```bash
cargo run --bin example
```
