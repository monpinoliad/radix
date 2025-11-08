# Radix

**Radix** is a simple embedded Rust project built for the [ESP32-S3-LCD-1.28](https://www.waveshare.com/wiki/ESP32-S3-LCD-1.28) development board. It leverages the power of `esp-hal` and optional GUI libraries to create a rich, bare-metal experience on an embedded device.

## 🛠 Features

- Developed in Rust using the `esp-hal` crate
- LCD display driven by the GC9A01 controller using [`gc9a01-rs`](https://github.com/almindor/gc9a01-rs)
- Optional GUI rendering via [`slint`](https://github.com/slint-ui/slint)
- Built to run on the ESP32-S3 microcontroller
