import * as fs from 'fs';
import {performance} from 'perf_hooks';

// import * as nail_salon from '../build';
// import {SimpleImageWorker} from "../build/worker_basic";
import {defaultOptions} from "../build/types";
import {ImageWorkerPool} from "../build/worker_pool";
import {EventEmitter, once} from "events";


function* genFiles(...dirs: string[]) {
  for (const dir of dirs) {
    if (!fs.existsSync(dir)) {
      console.error(`Image directory missing: ${dir}`);
      process.exit(1);
    }
    for (const filename of fs.readdirSync(dir)) {
      if (filename.startsWith('.') || filename.match(/(txt|csv)$/i))
        continue;
      yield [dir, filename];
    }
  }
}

async function main() {
  const tStart = performance.now();
  const out_dir = `${__dirname}/out`;
  const bad_dir = `${__dirname}/bad`;

  const threads = 8;
  const worker = new ImageWorkerPool(threads, 2000);
  // const worker = new SimpleImageWorker();

  fs.mkdirSync(out_dir, {recursive: true})
  fs.mkdirSync(bad_dir, {recursive: true})

  const stats = {
    failures: 0,
    successes: 0,
    timing: 0,
  };

  const src_dir = `${__dirname}/lcwa_gov_image_data/data`;

  let running = 0;
  const sync = new EventEmitter();

  const errLog = new Array<string>();

  for (const [in_dir, file] of genFiles(src_dir)) {
    const origPath = `${in_dir}/${file}`;
    const outPath = `${out_dir}/${file}`;
    const badPath = `${bad_dir}/${file}`;

    if (running > threads + 2) // keep the buffer full
      await once(sync, 'tick');

    running++;
    let imgStart: number;
    fs.promises.readFile(origPath)
      .then(raw => {
        const req = {
          ...defaultOptions,
          input: raw,  // TODO: Investigate extra failures when using raw.buffer vs buffer
          target_h: 512,
          target_w: 512,
        };
        imgStart = performance.now();

        // try {
        //   return Promise.resolve(nail_salon.convert(req));
        // } catch (e) {
        //   return Promise.reject(e);
        // }
        return worker.convert(req);
      })
      .then(res => {
        const timing = performance.now() - imgStart;
        stats.timing += timing;
        console.log(` + ${file} -- ${(timing).toFixed(3)}ms`);
        stats.successes++;
        return fs.promises.writeFile(outPath, res.output);
      }, e => {
        errLog.push(`${e.message || e} -- ${file}`);
        const timing = performance.now() - imgStart;
        stats.timing += timing;
        console.error(`ERR: ${file}`, e); //  -- ${(timing).toFixed(3)}ms`, e);
        stats.failures++;
        return fs.promises.copyFile(origPath, badPath);
      }).finally(() => {
        sync.emit('tick', --running)
      });
  }

  while (running > 0) {
    console.log(`waiting for ${running}...`);
    await once(sync, 'tick');
  }

  console.log(`Took ${performance.now() - tStart}ms`);
  console.dir(stats);
  console.log(errLog.sort().join('\n'));
}

if (require.main === module)
  main().then(() => process.exit(0), err => {
    console.error(err);
    process.exit(1);
  });
