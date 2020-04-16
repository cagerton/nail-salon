const fs = require('fs');
const {thumb_fix, fast_thumb_fix, init_panic_hook} = require('./pkg/rs');
init_panic_hook();

console.log('about to run', process.argv);
const raw = fs.readFileSync(process.argv[2])
const res = fast_thumb_fix(new Uint8Array(raw), 512);
// const res = thumb_fix(new Uint8Array(raw), 512);
fs.writeFileSync(process.argv[3], res);
// console.log('got result', raw.length, res.length);