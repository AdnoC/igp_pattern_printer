# IGP Pattern Printer

Takes a hex image created with the [Irregular Grid Painter](https://www.zlosk.com/pgmg/igp/index.html) and presents you "pixel"s at a time. Press a button to see the next few pixels. Saves your progress as you work.

Created to aid in creating pixel-art chainmail.

A web version is available at https://igp-pattern-printer.adno.page/

## TUI

A terminal UI is available.

### Usage

Run and pass in a filepath to the exported hex image.

```sh
cd tui
cargo run -- <FILENAME>
```
