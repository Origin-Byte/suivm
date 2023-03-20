# suivm

Sui Version Manager (`suivm`) is a tool for managing multiple versions of Sui CLI.

Sui publishes pre-compiled binaries which can be downloaded using `suivm` to speed up your workflow.

## Install

Install `suivm` using `cargo`:

```bash
cargo install --git https://github.com/origin-byte/suivm
```

Verify the installation:

```bash
suivm use latest
sui --version
```

`suivm` supports resolving tags, branches, and commit hashes:

```bash
suivm use devnet-0.27.0
suivm use devnet
suivm use 157ac72030d014f17d76cefe81f3915b4afab2c9
```

`suivm` will automatically download pre-compiled binary if a matching tag like `devnet-0.27.0` is provided.