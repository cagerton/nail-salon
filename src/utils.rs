use image::imageops::FilterType;

pub static VERSION: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Serialize)]
pub struct ImageInfo {
    pub format: String,
    pub animated: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize)]
pub struct ResizeRequest {
    #[serde(with = "serde_bytes")]
    pub input: Vec<u8>,
    pub resize_op: ResizeType,

    pub target_w: u16,
    pub target_h: u16,
    pub down_only: bool,

    pub jpeg_scaling: bool,
    #[serde(with = "FilterOption")]
    pub scale_filter: FilterType,

    pub output_format: OutputFormat,
    pub jpeg_quality: u8,

    pub support_animation: bool,
}

#[derive(Deserialize, Serialize)]
pub enum OutputFormat {
    JPEG,
    PNG,
    Auto,
}

#[derive(Deserialize)]
#[serde(remote = "FilterType")]
pub enum FilterOption {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

#[derive(Deserialize, PartialEq)]
pub enum ResizeType {
    Fit,
    Cover,
    Crop,
}

impl ResizeType {
    pub fn cover(&self) -> bool {
        !matches!(*self, ResizeType::Fit)
    }
}

#[derive(Serialize)]
pub struct ResizeResult {
    #[serde(with = "serde_bytes")]
    pub output: Vec<u8>,
    pub format: String,
    pub version: String,
    pub w: u16,
    pub h: u16,
}
