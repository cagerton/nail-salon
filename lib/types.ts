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

export const defaultOptions = Object.freeze({
    scale_filter: ScaleFilter.Lanczos3,
    jpeg_scaling: true,
    down_only: true,
    jpeg_quality: 80,
    resize_op: ResizeOp.Fit,
    output_format: OutputFormat.Auto,
  }
);

/**
 * Digest some common error messages.
 * @param msg
 */
export function simplifyError(msg: string) {
  if (msg.match(/ImageError.*UnsupportedError/)) {
    if (msg.match(/kind: Format\(Unknown\)/))
      return 'Unsupported image format';

    if (msg.match(/kind: Color/))
      return 'Unsupported pixel color type';

    if (msg.match(/Format\(Exact\(/))
      return 'Unsupported image format';

    if (msg.match(/kind: GenericFeature/))
      return 'Unsupported image feature';
  }

  if (msg.match(/ImageError.*DecodingError/) ||
      msg.match(/ParameterError/) ||
      msg.match(/InvalidInput/))
    return 'Failed to decode image';

  if (msg.match(/ImageError.*kind: UnexpectedEof/))
    return 'Unexpected end of file';
}
