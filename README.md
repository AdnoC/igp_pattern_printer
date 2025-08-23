# IGP Pattern Printer

Takes a hex image created with the [Irregular Grid Painter](https://www.zlosk.com/pgmg/igp/index.html) and presents you "pixel"s at a time. Press a button to see the next few pixels. Saves your progress as you work.

Created to aid in creating pixel-art chainmail.

A web version is available at https://igp-pattern-printer.adno.page/

## Organization

This project is broken up into 3 crates:

* `ipp`: The core logic and state machine. Initializes state with an image and updates internal state based on events. Provides a view of state for UI to use.
* `tui`: Terminal UI built on top of the [Ratatui UI framework](https://ratatui.rs/).
* `wasm`: Browser UI built using the React-like [Yew UI framework](https://yew.rs/).

## TUI

A terminal UI is available.

### Usage

Run and pass in a filepath to the exported hex image.

```sh
cd tui
cargo run -- <FILENAME>
```

## TODO

Implement touch controls
