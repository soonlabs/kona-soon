# Soon LiteSVM

This package contains the enhanced LiteSVM implementation for the Soon Network. It builds upon the open-source LiteSVM library with additional functionality and optimizations. We leverage and extend the original LiteSVM codebase for blockchain simulation needs within the Soon ecosystem.

We truly appreciate the open-source contributions from the LiteSVM community and their ongoing innovation in Solana VM simulation.

This project is licensed under the MIT License. See the LICENSE file for details.

Copyright (c) 2024 Soon Labs

## Core Modifications

We are currently synchronized with the upstream LiteSVM repository, and we maintain our own enhancements starting from commit [268c2a8](https://github.com/soonlabs/litesvm/commit/268c2a8). We will continue to incorporate upstream improvements while maintaining our specific enhancements.

### Key Enhancements

* **Account Management**: Enhanced account import/export functionality with improved error handling and retry mechanisms.
* **Fee Collection**: Added comprehensive fee collection and distribution logic for block processing.
* **Rent Epoch Management**: Improved rent epoch handling for account lifecycle management.
* **Block Processing**: Introduced batch transaction processing capabilities with fee collection.
* **Precompiles Support**: Added support for precompiled functions integration.
* **RISC0 Integration**: Adapted to use RISC0 forks for enhanced performance.
* **Decimal Precision**: Enhanced numerical precision for lamport calculations.
* **Error Handling**: Improved error logging and handling mechanisms.

## Original LiteSVM

This project is based on the original [LiteSVM](https://github.com/LiteSVM/litesvm) library, which provides a fast and lightweight Solana VM simulator for testing Solana programs. 