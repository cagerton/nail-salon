# NailSalon

NailSalon is a security-minded image thumbnailing module for NodeJS. It's based primarily on Rust's [image](https://crates.io/crates/image) crate and compiled to WebAssembly.

## Features
 * Immune to many classes of security vulnerabilities which commonly affect image processing libraries by nature of being written in a memory safe language and run in a WebAssembly sandbox with minimal surface area.
 * Preserves format for PNGs and JPEGs for all other supported types.
 * Automatically normalizes JPEG orientation using exif data.
 * Supports fast IDCT downscaling of JPEGs to improve performance.
 * TypeScript type definitions included.

## Typical usage
```TypeScript
import fs from 'fs';
import {scale_and_orient} from 'nail_salon';

const cover = true;
const downscale_only = true;

const orig = fs.readFileSync('example.jpg');
fs.writeFileSync('example.thumb.jpg', scale_and_orient(orig, 256, 256, cover, downscale_only));
```

## Building
```shell
wasm-pack build --release --target nodejs
```

## License
This software is distributed under the Apache License (Version 2.0)

