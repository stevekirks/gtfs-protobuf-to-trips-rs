# GTFS Protobuf to Trips (in Rust)

This project contains code to take a window of GTFS Realtime data and tranform it to deck.gl trips.

I wrote it in Rust as a learning experience.

### Usage
Clone this repo, set data specific settings in the AppSettings class in `main.rs`. Then:
```
cargo run
```

### Protobuf rust file generation
`gtfs_realtime.rs` was generated from 
```
extern crate protoc;
extern crate protoc_rust;

use protoc_rust::Codegen;

fn main() {
    Codegen::new()
        .protoc_path("C:/dev/temp/protobuf/protoc.exe")
        .out_dir("src/protos")
        .include("C:/dev/temp/protobuf/")
        .inputs(&["C:/dev/temp/protobuf/gtfs-realtime.proto"])
        .run()
        .expect("protoc");
}
```
and `Cargo.toml`
```
...
[dependencies]
protoc = "2.24"
protoc-rust = "2.24"
```