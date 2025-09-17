<p align="center">
  <a href="https://soo.network">
    <img alt="Kona-Soon" src="https://i.imgur.com/XugmfSD.png" width="630" />
  </a>
</p>

<h4 align="center">
    The SVM-Powered Monorepo for <a href="https://specs.optimism.io/">OP Stack</a> Components and Services built in Rust.
</h4>

<p align="center">
  <a href="#whats-kona-soon">What's Kona-Soon?</a> •
  <a href="#overview">Overview</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#msrv">MSRV</a> •
  <a href="https://op-rs.github.io/kona/CONTRIBUTING.html">Contributing</a> •
  <a href="#credits">Credits</a> •
  <a href="#license">License</a>
</p>


## What's Kona-Soon?

Kona-Soon is a revolutionary adaptation of the original [Kona](https://github.com/op-rs/kona) project that replaces the Ethereum Virtual Machine (EVM) with the **Solana Virtual Machine (SVM)** while leveraging the robust OP Stack L2 derivation pipeline. This breakthrough is achieved by adapting SOON network's SVM L2 derive compatibility capabilities into Kona's proven derivation framework, creating a unique hybrid architecture that seamlessly bridges OP Stack rollup infrastructure with Solana's high-performance execution environment.

The core innovation lies in SOON's sophisticated derive pipeline adaptation layer, which translates Solana block structures, account models, and transaction formats into formats compatible with Kona's L2 derivation process. This allows us to maintain the battle-tested OP Stack derivation logic while executing on SVM, effectively creating the first SVM-powered OP Stack rollup.

### Key Innovations

- **🔄 EVM → SVM Transition**: Complete replacement of the EVM executor with a lightweight, high-performance SVM implementation (`litesvm`)
- **🏗️ SOON Derive Pipeline Adaptation**: Leverages SOON's SVM L2 derive compatibility to seamlessly integrate Solana execution with OP Stack's proven L2 derivation framework
- **🛡️ Enhanced Hint/Oracle System**: Extended the original OP fault proof hint/oracle mechanism with SOON-specific data providers for L2 state verification
- **⚡ Risc0 zkVM Ready**: Built from the ground up for compilation to risc0 zkvm architecture, enabling zero-knowledge proof generation
- **🌐 SOON Network Integration**: Native integration with SOON network components for comprehensive L1/L2 chain management


## Architecture

> [!NOTE]
>
> Ethereum (Alloy) types modified for the OP Stack live in [op-alloy](https://github.com/alloy-rs/op-alloy).
> SOON-specific types and primitives live in the `soon-*` crates.

### Core Differences from Original Kona

| Component | Original Kona | Kona-Soon |
|-----------|---------------|-----------|
| **Execution Engine** | EVM (revm) | SVM (litesvm) |
| **Block Format** | Ethereum blocks | Solana blocks with L1 block info |
| **State Model** | Ethereum state trie | Solana account model + MPT integration |
| **Proof System** | EVM state proofs | SVM execution proofs + account proofs |

## Overview

**Binaries**

- [`client`](./bin/client): The bare-metal program that executes the SVM-based state transition, designed for risc0 zkvm execution.
- [`host`](./bin/host): The host program that runs natively alongside the prover, serving as the [Preimage Oracle][g-preimage-oracle] server with SOON-specific data providers.

**Protocol**

- [`genesis`](./crates/protocol/genesis): Genesis types for OP Stack chains.
- [`protocol`](./crates/protocol/protocol): Core protocol types used across OP Stack rust crates.
- [`comp`](./crates/protocol/comp): Compression and decompression utilities for the derivation pipeline.
- [`driver`](./crates/protocol/driver): Stateful derivation pipeline driver adapted for SVM execution.

**Execution**

- [`fraud-executor`](./crates/executor/fraud): SVM-based block executor that integrates with the OP Stack derivation pipeline.
- [`litesvm`](./crates/executor/svm): Lightweight Solana Virtual Machine implementation optimized for proof generation.

**Proof**

- [`mpt`](./crates/proof/mpt): Utilities for interacting with the Merkle Patricia Trie in the client program.
- [`proof`](./crates/proof/proof): High-level OP Stack state transition proof SDK with SVM integration.
- [`preimage`](./crates/proof/preimage): High-level interfaces to the [`PreimageOracle`][fpp-specs] ABI with SOON-specific hint types.
- [`std-fpvm`](./crates/proof/std-fpvm): Platform-specific [Fault Proof VM][g-fault-proof-vm] kernel APIs.
- [`std-fpvm-proc`](./crates/proof/std-fpvm-proc): Proc macro for [Fault Proof Program][fpp-specs] entrypoints.

**SOON Integration**

- **soon-derive**: L2 derivation pipeline adapted for SOON network block structures
- **soon-primitives**: Core data types and structures for SOON network integration
- **soon-l1-chain-provider**: L1 chain data provider for SOON network
- **soon-l2-chain-provider**: L2 chain data provider with SVM state management
- **soon-da-provider**: Data availability provider for SOON network
- **soon-mpt-trie**: Merkle Patricia Trie implementation optimized for SOON

**Utilities**

- [`serde`](./crates/utilities/serde): Serialization helpers.
- [`cli`](./crates/utilities/cli): Standard CLI utilities, used across `kona-soon`'s binaries.

### SVM Integration Details

The SVM integration in Kona-Soon introduces several key enhancements:

#### Enhanced Hint Types
- **`L2BlockData`**: Fetches complete Solana block data including transaction metadata
- **`L2BankHash`**: Retrieves Solana bank hash for state verification
- **`L2BlockTime`**: Provides block timing information for Solana blocks
- **`L2AccountProof`**: Fetches account state and existence proofs in a single request

#### State Management
- **Account Model Integration**: Seamless mapping between Solana's account model and OP Stack's state requirements
- **Bank Hash Tracking**: Integration of Solana's bank hash concept for state commitment
- **Cross-Chain State Proofs**: Enhanced proof generation for cross-chain state verification

### Proof System

Built on top of these libraries, this repository features a [proof program][fpp-specs] designed to deterministically execute the SVM-based rollup state transition in order to verify an [L2 output root][g-output-root] from the L1 inputs it was [derived from][g-derivation-pipeline].

Kona-Soon's libraries were built with risc0 zkvm compatibility and extensibility in mind. The repository features a fault proof virtual machine backend that integrates SVM execution with the governance-approved OP Stack infrastructure.


## MSRV

The current MSRV (minimum supported rust version) is `1.85`.

The MSRV is not increased automatically, and will be updated only as part of a patch (pre-1.0) or minor (post-1.0) release.

## Crate Releases

`kona-soon` releases are done using the [`cargo-release`](https://crates.io/crates/cargo-release) crate. A detailed guide is available in [./RELEASES.md](./RELEASES.md).

## Contributing

`kona-soon` is built by open source contributors like you, thank you for improving the project!

A [contributing guide][contributing] is available that sets guidelines for contributing.

Pull requests will not be merged unless CI passes, so please ensure that your contribution follows the linting rules and passes clippy.

## Dependencies

### SOON Network Components
- **Solana SDK**: Custom fork optimized for risc0 zkvm compilation
- **SOON Derive**: L2 derivation pipeline adapted for Solana block structures
- **LiteSVM**: Lightweight Solana Virtual Machine for efficient execution

### Risc0 Integration
All cryptographic dependencies are patched to use risc0-compatible versions:
- `curve25519-dalek`: risc0-compatible elliptic curve operations
- `sha2`: risc0-optimized SHA-2 implementation
- `k256`: risc0-compatible secp256k1 operations

## Credits

`kona-soon` is inspired by and builds upon the work of several teams:
- [OP Labs][op-labs] and contributors' work on the [Optimism monorepo][op-go-monorepo]
- The original [Kona][kona-original] project by the OP Rust community
- [SOON Network](https://soo.network) for the SVM integration architecture
- [Risc0](https://risczero.com) for the zero-knowledge proof system

`kona-soon` is built on rust types in [alloy][alloy], [op-alloy][op-alloy], and Solana SDK.

## License

Licensed under the [MIT license.](https://github.com/op-rs/kona/blob/main/LICENSE.md)

> [!NOTE]
>
> Contributions intentionally submitted for inclusion in these crates by you
> shall be licensed as above, without any additional terms or conditions.


<!-- Links -->

[alloy]: https://github.com/alloy-rs/alloy
[op-alloy]: https://github.com/alloy-rs/op-alloy
[contributing]: https://op-rs.github.io/kona/CONTRIBUTING.html
[op-stack]: https://github.com/ethereum-optimism/optimism
[superchain-registry]: https://github.com/ethereum-optimism/superchain-registry
[op-go-monorepo]: https://github.com/ethereum-optimism/optimism/tree/develop
[cannon]: https://github.com/ethereum-optimism/optimism/tree/develop/cannon
[cannon-rs]: https://github.com/op-rs/cannon-rs
[rollup-node-spec]: https://specs.optimism.io/protocol/rollup-node.html
[badboi-cannon-rs]: https://github.com/BadBoiLabs/cannon-rs
[asterisc]: https://github.com/etheruem-optimism/asterisc
[fpp-specs]: https://specs.optimism.io/fault-proof/index.html
[kona-original]: https://github.com/op-rs/kona
[op-labs]: https://github.com/ethereum-optimism
[bad-boi-labs]: https://github.com/BadBoiLabs
[g-output-root]: https://specs.optimism.io/glossary.html#l2-output-root
[g-derivation-pipeline]: https://specs.optimism.io/protocol/derivation.html#l2-chain-derivation-pipeline
[g-fault-proof-vm]: https://specs.optimism.io/experimental/fault-proof/index.html#fault-proof-vm
[g-preimage-oracle]: https://specs.optimism.io/fault-proof/index.html#pre-image-oracle