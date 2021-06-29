# rust-dynamic-plugin-system

Thanks for [luojia65/plugin-system-example](https://github.com/luojia65/plugin-system-example)

Example design of dynamic compile `dylib` based plugin on runtime in Rust, load and run it.

**Warning**: the source code is tested and run on MacOS, if your system is not the same, you should edit the code's `.dylib` to another:

* Linux: `.so`
* Windows: `.dll`
* MacOS: `.dylib`

## Usage

1. Compile and run main: `cargo run -p my-master`
