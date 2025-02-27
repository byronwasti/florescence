# Florescence [WIP]

An experimental library for hybrid Raft and CRDT primitives.

*The following is all a demo of what the library will look like and is not implement yet aside from type system sketches.*


## Example 1: Rate Limiting
```rust
// Start up the Flower
let flower = Flower::builder()
    .engine(ToniRpc::new("0.0.0.0:8070".parse()?))
    .bloom().await?;

// Attach various pollinators
let rate_limiter = flower.streaming_pollinator::<IdentityMap<u64>>();


// Kick off leaky-bucket background job
tokio::spawn(async || {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        rate_limiter.apply(|x| x - 1);
    }
})


// on Request, do a fully local lookup
fn handle_request(..) {
    rate_limiter.apply(|x| x + 1);
    let rate = rate_limiter.fold(|acc, x| acc + x);

    if rate > CONFIG_LIMIT {
        return Err(Http503);
    }
}
```


## Example 2: Ratcheting

```rust
```

## License

Licensed under either of:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
