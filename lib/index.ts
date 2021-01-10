import {OutputFormat, ResizeOp, ScaleFilter, ResizeRequest, ResizeResult, ImageInfo} from './types';

export * from './types';
export {ImageWorkerPool} from './image_worker_pool';

import type WasmModule from '../wasm/nail_salon';

// Defer wasm compilation overhead until first use
let cachedModule: WasmModule.ExposedFunctions;

export function convert(request: ResizeRequest): ResizeResult {
  if (!cachedModule)
    cachedModule = require('../wasm/nail_salon');

  return cachedModule.convert(request);
}

export function version() {
  if (!cachedModule)
    cachedModule = require('../wasm/nail_salon');

  return cachedModule.version();
}

export function image_info(input: Uint8Array): ImageInfo {
  if (!cachedModule)
    cachedModule = require('../wasm/nail_salon');

  return cachedModule.image_info(input);
}

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
