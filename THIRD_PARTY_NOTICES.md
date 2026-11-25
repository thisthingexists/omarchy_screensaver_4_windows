# Third-party notices

MyScreensaver's effects come entirely from **ttfx**. ttfx is compiled into
`MyScreensaver.scr` as a library, pinned to commit `14ed274` and unmodified.
ttfx is a Rust port of **TerminalTextEffects**. Both are MIT-licensed, and
their notices are reproduced below as their licenses require.

Other Rust crates linked into the binary (`clap`, `windows-sys`,
`unicode-width`, `terminal_size` and their dependencies) are MIT or
MIT/Apache-2.0 licensed. Run `cargo tree` to list them.

---

## ttfx

https://github.com/omacom/ttfx

```
MIT License

Copyright (c) 2026 37signals / omacom-io
Copyright (c) 2023 ChrisBuilds (TerminalTextEffects, from which this is ported)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## TerminalTextEffects

https://github.com/ChrisBuilds/terminaltexteffects

```
MIT License

Copyright (c) [2023] [ChrisBuilds]

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Omarchy

The *idea* of this screensaver is the one in Omarchy
(https://github.com/basecamp/omarchy, `bin/omarchy-screensaver` and
`bin/omarchy-launch-screensaver`). No Omarchy code or artwork is included.
The default art is original.
