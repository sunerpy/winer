# Third-party notices

winer is MIT licensed (`LICENSE`). It ships the following third-party component in its binaries,
under that component's own licence.

## Pengu Loader

`vendor/pengu-loader/core.dll`, from [Pengu Loader](https://github.com/PenguLoader/PenguLoader)
v1.1.6, is embedded in winer and written to winer's data folder to load the in-client plugin
(`vendor/pengu-loader/README.md`).

```text
MIT License

Copyright (c) 2024 Pengu Loader

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

## Libraries

The Rust crates and npm packages winer is built from are listed with their versions in
`Cargo.lock` and `pnpm-lock.yaml`, each under its own licence. For the Windows build's Rust crates
those are MIT and Apache-2.0 for almost all of them, plus BSD-3-Clause, ISC, Zlib and Unicode-3.0,
and MPL-2.0 for `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext` and `selectors`, which
are used unmodified; their source is on crates.io at the versions `Cargo.lock` names.
