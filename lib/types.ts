export enum ScaleFilter {
  Nearest = 'Nearest',
  Triangle = 'Triangle',
  CatmullRom = 'CatmullRom',
  Gaussian = 'Gaussian',
  Lanczos3 = 'Lanczos3',
}

export enum ResizeOp {
  Fit = 'Fit',
  Cover = 'Cover',
  Crop = 'Crop',
}

export enum OutputFormat {
  JPEG = 'JPEG',
  PNG = 'PNG',
  Auto = 'Auto',
}

export interface ResizeRequest {
  resize_op: ResizeOp;
  input: ArrayBufferLike;
  target_w: number;
  target_h: number;
  down_only: boolean;
  jpeg_scaling: boolean;
  scale_filter: ScaleFilter;
  output_format: OutputFormat;
  jpeg_quality: number;
}

export interface ResizeResult {
  output: ArrayBufferLike;
  format: OutputFormat;
  w: number;
  h: number;
}

export interface ImageInfo {
  format: string;
  width: number;
  height: number;
}

export interface WorkerRequest {
  taskId: number;
  req: ResizeRequest;
}

export interface WorkerResult {
  taskId: number,
  res?: ResizeResult,
  err?: Error,
}

/**
 * Digest some common error messages.
 * @param msg
 */
export function simplifyError(msg: string) {
  if (msg.match(/ImageError.*UnsupportedError/)) {
    if (msg.match(/kind: Format\(Unknown\)/))
      return 'unknown image format';

    if (msg.match(/kind: Color/))
      return 'unsupported color type';

    if (msg.match(/Format\(Exact\(/))
      return 'unsupported image format';

    if (msg.match(/kind: GenericFeature/))
      return 'unsupported image feature';
  }

  if (msg.match(/ImageError.*DecodingError/))
    return 'error decoding image';

  if (msg.match(/ImageError.*kind: UnexpectedEof/))
    return 'image file incomplete';
}
