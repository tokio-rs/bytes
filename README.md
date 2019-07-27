# Bytes

A utility library for working with bytes.

[![Crates.io](https://img.shields.io/crates/v/bytes.svg?maxAge=2592000)](https://crates.io/crates/bytes)
[![Build Status](https://dev.azure.com/tokio-rs/bytes/_apis/build/status/tokio-rs.tokio?branchName=master)](https://dev.azure.com/tokio-rs/bytes/_build/latest?definitionId=1&branchName=master)

[Documentation](https://docs.rs/bytes)

## Usage

To use `bytes`, first add this to your `Cargo.toml`:

```toml
[dependencies]
bytes = "0.4.12"
```

Next, add this to your crate:

```rust
use bytes::{Bytes, BytesMut, Buf, BufMut};
```

## Serde support

Serde support is optional and disabled by default. To enable use the feature `serde`.

```toml
[dependencies]
bytes = { version = "0.4.12", features = ["serde"] }
```

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `bytes` by you, shall be licensed as MIT, without any additional
terms or conditions.

