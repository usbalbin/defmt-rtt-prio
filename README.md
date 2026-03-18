# `defmt-rtt-prio`

> Transmit [`defmt`] log messages over the RTT (Real-Time Transfer) protocol without blocking

This is based on defmt-rtt from [knurling-rs/defmt](https://github.com/knurling-rs/defmt).However `defmt-rtt-prio` avoids any critical sections by exploiting the fact that interrupts of the same priority can not interrupt each other. We setup one RTT UP channel per NVIC priority level. By mapping each priority to its own RTT channel, we can guarantee that there will be no problems.

NOTE when using this crate it's not possible to use (link to) the
`defmt-rtt` or `rtt-target` crates

To use this crate, link to it by importing it somewhere in your project.

```
// src/main.rs or src/bin/my-app.rs
use defmt_rtt_prio as _;
```

[`defmt`]: https://github.com/knurling-rs/defmt

`defmt` ("de format", short for "deferred formatting") is a highly efficient logging framework that targets resource-constrained devices, like microcontrollers.

For more details about the defmt framework check the book at <https://defmt.ferrous-systems.com>.

## Memory use

When in a tight memory situation and logging over RTT, the buffer size (default: 1024 bytes) can be configured with the `DEFMT_RTT_BUFFER_SIZE` environment variable. Use a power of 2 for best performance.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)

- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
licensed as above, without any additional terms or conditions.
