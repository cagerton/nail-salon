use wasm_bindgen::JsValue;

#[derive(Debug)]
pub enum MultiErr {
    ImageError(image::error::ImageError),
    GifError(gif::EncodingError),
    ExifErr(exif::Error),
    TryFromIntError(std::num::TryFromIntError),
}

impl Into<JsValue> for MultiErr {
    fn into(self) -> JsValue {
        JsValue::from(format!("{:?}", self))
    }
}

impl From<image::error::ImageError> for MultiErr {
    fn from(err: image::error::ImageError) -> MultiErr {
        MultiErr::ImageError(err)
    }
}

impl From<gif::EncodingError> for MultiErr {
    fn from(err: gif::EncodingError) -> MultiErr {
        MultiErr::GifError(err)
    }
}

impl From<exif::Error> for MultiErr {
    fn from(err: exif::Error) -> MultiErr {
        MultiErr::ExifErr(err)
    }
}

impl From<std::num::TryFromIntError> for MultiErr {
    fn from(err: std::num::TryFromIntError) -> MultiErr {
        MultiErr::TryFromIntError(err)
    }
}
