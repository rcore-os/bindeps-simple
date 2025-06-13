# Bindeps

Bindeps is used to get the dependencies as bin, with more options in stable channel.

## Usage

Bin type dependency should have a `lib.rs` file, or it will be ignored. 

```toml
[dependencies]
foo = { path = "path/to/foo", version = "0.1" }

[build-dependencies]
bindeps-simple = {version = "*"}
```

In `build.rs`:

```rust
let output = bindeps_simple::Builder::new("foo").build().unwrap();
let _ = output.elf;
```

The path of `foo` bin is in `output.elf`.
