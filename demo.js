const fs = require('fs');
const nails = require('./pkg/nail_salon');

if (process.argv.length < 4) {
    console.error('Usage: node test.js <input> <output>');
    process.exit(1);
}

const raw = new Uint8Array(fs.readFileSync(process.argv[2]));

try {
    const {width, height} = nails.dimensions(raw);
    console.log("input: width:", width, "height:", height);

    const res = nails.scale_and_orient(raw, 162, 246);
    console.log("thumbnail: width:", res.width, "height:", res.height);
    fs.writeFileSync(process.argv[3], res.thumbnail());
} catch (e) {
    console.log(process.argv[2])
    console.log("Error:");
    console.dir(e);
}
