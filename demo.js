const fs = require('fs');
const nails = require('./nail_salon');

if (process.argv.length < 4) {
    console.error('Usage: node test.js <input> <output>');
    process.exit(1);
}

nails.init_panic_hook();
const raw = fs.readFileSync(process.argv[2])

try {
    const res = nails.scale_and_orient(new Uint8Array(raw), 512);
    if (res)
        fs.writeFileSync(process.argv[3], res);
} catch (e) {
    console.log(process.argv[2])
    console.log("Error:");
    console.dir(e);
}
