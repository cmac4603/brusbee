# brusbee

Wifi AP, run things kinda like BadUSB.

Currently only runs on ESP32C5, but trivial to add more if needed.

## Running locally

Use [`bacon`](https://dystroy.org/bacon/) and the default job run should flash to the chip as long as it is seen (check with `espflash board-info`)

```bash
bacon
```

If `bacon` is not installed, you can use the Makefile:

```bash
make run
```

## TODO

- [x] Create wifi AP
- [x] Webserver running with [picoserve](https://crates.io/crates/picoserve)
- [ ] Admin page
- [ ] Run a command on host as if HID keyboard
- [ ] Captive portal
- [ ] Run Duckyscript (unlikely to get to, but let's see how it goes)
