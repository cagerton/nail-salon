import {convert} from '../wasm/nail_salon';
import {OutputFormat, ResizeOp, ScaleFilter} from './types';

export * from './types';
export {ImageWorkerPool} from './image_worker_pool';
export {convert, image_info} from "../wasm/nail_salon";

/**
 * @deprecated use convert instead
 */
export function scale_and_orient(
  input: Uint8Array,
  target_w: number,
  target_h: number,
  cover: boolean,
  down_only: boolean,
): ArrayBufferLike {
  const res = convert({
    input,
    target_h,
    target_w,
    down_only,
    scale_filter: ScaleFilter.CatmullRom,
    jpeg_scaling: true,
    jpeg_quality: 80,
    resize_op: cover ? ResizeOp.Cover : ResizeOp.Fit,
    output_format: OutputFormat.Auto,
  });
  return res.output;
}
