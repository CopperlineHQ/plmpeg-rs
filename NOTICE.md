# Source lineage

The MPEG-1 reconstruction core in `src/decoder.rs` and its bit-reader ancestry
derive from the pure-Rust MPEG implementation in
[Native32Emu](https://github.com/AloysHF/Native32Emu), retrieved at commit
`923f6da0847281eeb684c2216d76e5eb2171f527`. Native32Emu is copyright Aloys
(AloysHF) and distributed under the BSD 3-Clause License in `LICENSE`.

Native32Emu's implementation is a Rust port of
[PL_MPEG](https://github.com/phoboslab/pl_mpeg) by Dominic Szablewski, an MIT
licensed MPEG-1 video and MPEG Layer II decoder. The upstream source retrieved
at commit `c871f2be022ece7ef4f64230b4fb8e1fb9eb6023` identifies that license with
`SPDX-License-Identifier: MIT`; its license notice is retained in
`LICENSE-PL_MPEG`.

This crate extracts only MPEG-1 video decoding, adds an incremental and bounded
input model, error handling, resource limits, a reusable API, and independent
tests. It contains no Native32 game-emulation code and no C source.

