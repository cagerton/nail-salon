import * as fs from 'fs';
import * as nail_salon from '../build';
import {performance} from 'perf_hooks';
import {OutputFormat, ResizeOp, ScaleFilter} from "../lib/types";


async function main() {
    const in_dir = `${__dirname}/lcwa_gov_image_data/data`;
    const out_dir = `${__dirname}/out`;
    const bad_dir = `${__dirname}/bad`;

    if (!fs.existsSync(in_dir)) {
        console.error('Run setup_bench_data.ts and try again');
        process.exit(1);
    }

    fs.mkdirSync(out_dir, {recursive: true})
    fs.mkdirSync(bad_dir, {recursive: true})

    const stats = {
        failures: 0,
        successes: 0,
        timing: 0,
    };

    const files = fs.readdirSync(in_dir)
        .filter(name => !(name.match(/(txt|csv)$/i)))
        ;

    for (const file of files) {
        const origPath = `${in_dir}/${file}`;
        const outPath = `${out_dir}/${file}`;
        const badPath = `${bad_dir}/${file}`;

        const raw = fs.readFileSync(origPath);

        let tStart = performance.now();
        try {
            // const res = nail_salon.convert({
            //
            const res = await nail_salon.convert({
              input: raw,
              target_h: 512,
              target_w: 512,
              down_only: true,
              scale_filter: ScaleFilter.CatmullRom,
              jpeg_scaling: true,
              jpeg_quality: 80,
              resize_op: ResizeOp.Cover,
              output_format: OutputFormat.Auto,
            });
            // const thumb = nail_salon.scale_and_orient(raw, 512, 512, true, true);
            // let info = nail_salon.image_info(thumb);
            // const {output, ...info} = res;
            // console.log(info);

            const timing = performance.now() - tStart;
            stats.timing += timing;
            console.log(` + ${file} -- ${(timing).toFixed(3)}ms`);
            stats.successes++;
            fs.writeFileSync(outPath, res.output);

        } catch (e) {
            const timing = performance.now() - tStart;
            stats.timing += timing;
            console.error(`ERR: ${file} -- ${(timing).toFixed(3)}ms`, e);
            fs.copyFileSync(origPath, badPath);
            stats.failures++;
        }
    }
    console.dir(stats);
}
if (require.main === module)
    main();
