# brusbee

Wifi Targeted Active PMKID Sniffer.

Currently tested on ESP32C5.

## Running locally

Use [`bacon`](https://dystroy.org/bacon/) and the default job run should flash to the chip as long as it is seen (check with `espflash board-info`)

```bash
bacon
```

If `bacon` is not installed, you can use the Makefile:

```bash
make run
```

## Updating styling

The web UI uses [tailwind](https://tailwindcss.com/) & [daisyUI](https://daisyui.com/), to update when adding new classes:

```bash
npm run build:css
```

## TODO

- [x] Create wifi AP
- [x] Webserver running with [picoserve](https://crates.io/crates/picoserve)
- [x] Homepage with assets
- [ ] Run a command on host as if HID keyboard
- [ ] Captive portal
- [ ] Run Duckyscript (unlikely to get to, but let's see how it goes)
