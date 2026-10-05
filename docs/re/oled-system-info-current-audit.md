# 691 OLED System Information editor audit

The source is the current product 691 dashboard extracted under
`.ref/devices/691/` (2026-10-02). The editor is `CustomizeSystemInfo` in
`.ref/devices/691/static/js/OLED.b7b95581.chunk.js`; its source action carries
`timeBetweenSlides`, `temperatureUnit`, `timeFormat`, `dateFormat`, and three
two-slot `slides` arrays. The default reducer state is in
`.ref/devices/691/static/js/main.e51ce83a.js` around byte offset 725005:

* intervals: `3`, `5`, `10` seconds;
* temperature: `celsius` or `fahrenheit`;
* date formats: `mm/dd/yyyy`, `dd/mm/yyyy`, `yyyy/mm/dd`;
* time formats: `12H` and `24H`;
* default slides: CPU usage/temperature, GPU usage/temperature, date/time.

The tag protocol types and product-specific keyboard-battery extension are
defined by module `96373` in `main.e51ce83a.js` (the product-691 source bundle
also imports that table). The Rust editor retains these IDs and numeric types,
stages all changes in a local dialog draft, and writes `/oled/system` only on
Apply. Telemetry values remain service-owned; the preview uses the audited
default values as a local visual stand-in.

The source references these original preview assets, which are not currently
registered in `assets/synapse/`:

* `static/media/cpu_usage.40da1439.svg`
* `static/media/cpu_temp.b9448451.svg`
* `static/media/gpu_usage.fc23e5e9.svg`
* `static/media/gpu_temp.e4e80122.svg`
* `static/media/memory.439f4636.svg`
* `static/media/date.acc342cb.svg`
* `static/media/time.c85d4a49.svg`
* `static/media/laptop_battery.2671c3ec.svg`
* `static/media/keyboard_battery.a1303ede.svg`

The editor intentionally does not invent replacement files. The `/oled/system` profile schema is now part of the product-691 descriptor, and the System Info card Edit action opens `SourceControls::open_oled_system`. Runtime telemetry, device transfer, and the original preview SVGs remain outside the local UI boundary.
