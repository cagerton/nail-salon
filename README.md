# NailSalon

NailSalon's objective is to provide a safe and performant library for the server-side generation of thumbnails for common image files in NodeJS.

## Background

Generating thumbnails is dangerous in general, and for a NodeJS developer, its especially so.
Security vulnerabilities are extremely common in core image libraries such as [libpng](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=libpng), [libjpeg](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=libjpeg), or [libexif](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=libexif).
These percolate up to image processing libraries and utilities such as [ImageMagick](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=imagemagick), [GraphicsMagick](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=graphicsmagick), [libvips](https://cve.mitre.org/cgi-bin/cvekey.cgi?keyword=libvips).
When it comes time to handle images in a NodeJS application, your main options are to use libraries that wrap the command line tools or native modules which use the error-prone libraries.
Command line wrappers introduce fun opportunities for [command injection](https://snyk.io/vuln/npm:gm) attacks and native modules carry all kinds of baggage around install time compilation or whatever prebuilds happen to be available.

Even supposing you manage to keep on top of known vulnerabilities, it's probably a good idea to think about various forms of isolation and sandboxing to mitigate unknown ones as well. This gets complex quickly and tends to be quite platform specific.

NailSalon avoids most common image processing vulnerabilities by using libraries that were written in a memory safe language and applies robust, platform agnostic sandboxing by running in a WebAssembly VM with limited surface area.

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

## License
Apache License (Version 2.0)

