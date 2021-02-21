use wasm_bindgen::JsValue;

macro_rules! error_enum {
    (pub enum $name:ident {$(
      $var:ident($t:ty)
    ),+}) => {
        #[derive(Debug)]
        pub enum $name {
          $($var($t),)+
        }

        impl Into<JsValue> for $name {
            fn into(self) -> JsValue {
                JsValue::from(format!("{:?}", self))
            }
        }

        $(
            impl From<$t> for $name {
                fn from(err: $t) -> Self {
                    Self::$var(err)
                }
            }
        )+
    }
}

error_enum! {
    pub enum MultiErr {
        ImageError(image::error::ImageError),
        ExifErr(exif::Error),
        TryFromIntError(std::num::TryFromIntError),
        GifEncodingError(gif::EncodingError),
        GifDecodingError(gif::DecodingError)
    }
}
