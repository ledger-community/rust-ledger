# Ledger hardware wallet communication library

A rust-based library for interacting with Ledger hardware wallets.
This provides low-level USB/HID, BLE, and TCP/Speculos `Transport`s as well as a high level `LedgerProvider` interface that manages device connections using a pinned worker thread for use from async / tokio contexts.

## Status

[![CI](https://github.com/ledger-community/rust-ledger/actions/workflows/ci.yml/badge.svg)](https://github.com/ledger-community/rust-ledger/actions/workflows/ci.yml)
[![GitHub tag](https://img.shields.io/github/tag/ledger-community/ledger.svg)](https://github.com/ledger-community/rust-ledger)
[![Latest docs](https://img.shields.io/badge/docs-latest-blue)](https://ledger-community.github.io/rust-ledger/ledger_lib/index.html)

This project is under active development, if you run into bugs please feel free to open an issue or PR.

## Layout

- [ledger-lib](lib) provides a library for communication with ledger devices  
  [![Crates.io](https://img.shields.io/crates/v/ledger-lib.svg)](https://crates.io/crates/ledger-lib) [![Docs.rs](https://docs.rs/ledger-lib/badge.svg)](https://docs.rs/ledger-lib)
- [ledger-proto](proto) provides shared APDU / protocol traits and objects  
  [![Crates.io](https://img.shields.io/crates/v/ledger-proto.svg)](https://crates.io/crates/ledger-proto) [![Docs.rs](https://docs.rs/ledger-proto/badge.svg)](https://docs.rs/ledger-proto)
- [ledger-cli](cli) provides a simple command line utility for interacting with ledger devices  
  [![Crates.io](https://img.shields.io/crates/v/ledger-cli.svg)](https://crates.io/crates/ledger-cli) [![Docs.rs](https://docs.rs/ledger-cli/badge.svg)](https://docs.rs/ledger-cli)
- [ledger-sim](sim) provides a rust wrapper to simplify use of [Speculos] for CI/CD  
  [![Crates.io](https://img.shields.io/crates/v/ledger-sim.svg)](https://crates.io/crates/ledger-sim) [![Docs.rs](https://docs.rs/ledger-sim/badge.svg)](https://docs.rs/ledger-sim)


[speculos]: https://github.com/LedgerHQ/speculos
