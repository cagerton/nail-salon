export enum ScaleFilter {
  Nearest = 'Nearest',
  Triangle = 'Triangle',
  CatmullRom = 'CatmullRom',
  Gaussian = 'Gaussian',
  Lanczos3 = 'Lanczos3',
}

export enum ResizeOp {
  /**
   * Scale the input to fit within the target dimensions.
   */
  Fit = 'Fit',

  /**
   * Scale an input to cover the target dimensions.
   */
  Cover = 'Cover',

  /**
   * Scale the input to cover the target dimensions and trim the excess.  Keeps the central area.
   */
  Crop = 'Crop',
}

export enum OutputFormat {
  JPEG = 'JPEG',
  PNG = 'PNG',
  GIF = 'GIF',

  /**
   * Uses the PNG encoder if the input was a PNG. Otherwise uses a JPEG encoder.
   */
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

export const defaultOptions = Object.freeze({
    scale_filter: ScaleFilter.Lanczos3,
    jpeg_scaling: true,
    down_only: true,
    jpeg_quality: 80,
    resize_op: ResizeOp.Fit,
    output_format: OutputFormat.Auto,
  }
);

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
 * Summarize some common error message strings.
 * @param msg
 */
export function simplifyError(msg: string): string | undefined {
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
