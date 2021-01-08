# Rust SDP800 Driver

![No Std][no-std-badge]

This is a platform agnostic Rust driver for the Sensirion SDP800 differential pressure sensor.
using the [`embedded-hal`](https://github.com/japaric/embedded-hal) traits.

## The Device

The Sensirion SDP800 is a differential pressure sensor. It has an I²C interface.

## Status

- [x] Get product identifier
- [x] Get differential pressure sensor (triggered sample)
- [ ] Mass flow / differential pressure measurement selection
- [x] Calculation
- [ ] Type-state based initialization and mode selection (triggered and continuous mode, idle mode)

## License

Licensed under

 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

<!-- Badges -->
[no-std-badge]: https://img.shields.io/badge/no__std-yes-blue
