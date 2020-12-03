# NailSalon

NailSalon's objective is to provide a safe and performant library for the server-side generation of thumbnails for common image files in NodeJS.

## Background

Safe thumbnail generation for NodeJS applications is hard. Most relevant NPM modules use native image processing libraries or wrap command line utilities which have a long history of security vulnerabilities. Some JavaScript-only libraries are available, but they tend to be very slow. NailSalon avoids these problems using libraries which are written in Rust and running them in a WebAssembly VM.

## Typical usage
```typescript
import fs from 'fs';
import {ImageWorkerPool, defaultOptions} from 'nail-salon';
import path from 'path';

// Move work off of the main thread and impose time limits using an ImageWorkerPool
const pool = new ImageWorkerPool(2);

async function handleImage(original: string) {
  const {output, ...details} = await pool.convert({
    ...defaultOptions,
    input: fs.readFileSync(original),
    target_h: 256,
    target_w: 256,
  });
  const {dir, name} = path.parse(original);
  const destination = `${dir}/${name}_thumb.${details.format.toLocaleLowerCase()}`;
  console.dir({original, destination, ...details, outputSize: output.byteLength});
  fs.writeFileSync(destination, output);
}

Promise.all(['./example1.jpg', './example2.png', './example3.gif'].map(handleImage))
  .then(() => process.exit(0), err => {
    console.error(err);
    process.exit(1);
  });
```

## Building
See build.sh or GitHub action for details.

## Testing
I've included a benchmarking script uses 1000 sample images collected by the Library of Congress. ([corpus details](https://lclabspublicdata.s3.us-east-2.amazonaws.com/lcwa_gov_image_README.txt))

```shell
npm i
./setup_bench_data.sh
node -r ts-node/register bench/bench.ts
```

## Changes

### 0.2.6
* Use 8bit pixel depth for thumbnail encoding. Fixes an issue that was reported with 16bit depth PNG images.
* Bump dependencies

### 0.2.5
* Add `main` to `package.json` - thanks

### 0.2.4
* Defer Wasm compilation until necessary

### 0.2.3
* Fix scale_dimensions for extremely narrow images
* Add support for cropping, quality controls, and additional interface options through `convert(...)`
* Deprecate `scale_and_orient`, which is now implemented using `convert(...)`
* Switch to using `serde_wasm_bindgen` and manual TypeScript types + helpers
* Add additional build steps using `build.sh` to support the extra TypeScript
* Introduce the `ImageWorkerPool` which supports concurrent workers and execution limits
* Limit jpeg scaling to 2x the image size to improve resize quality, set Lanczos3 as the default filter

## License
Apache License (Version 2.0)

## Links
 * This was partially inspired by [Squoosh](https://squoosh.app/), which leverages WebAssembly to power an image codec testing web application.
