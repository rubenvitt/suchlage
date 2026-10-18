//! Vorschaubild eines Fotos: verkleinertes JPEG. Rein und blockierend, der Aufrufer legt es auf
//! `spawn_blocking` (ein Foto der H20N hat bis 20 MP).

use std::io::Cursor;

use image::ImageFormat;
use image::codecs::jpeg::JpegEncoder;

/// Längste Seite des Vorschaubilds in Pixeln.
pub const EDGE_PX: u32 = 640;
const QUALITY: u8 = 80;

pub fn make(original: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory_with_format(original, ImageFormat::Jpeg)
        .map_err(|e| format!("Foto nicht lesbar: {e}"))?;
    // Ein Wärmebild (640 × 512) ist schon klein genug; `thumbnail` würde es vergrößern.
    let small = if img.width() <= EDGE_PX && img.height() <= EDGE_PX {
        img.into_rgb8()
    } else {
        img.thumbnail(EDGE_PX, EDGE_PX).into_rgb8()
    };
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, QUALITY)
        .encode_image(&small)
        .map_err(|e| format!("Vorschaubild nicht schreibbar: {e}"))?;
    Ok(out.into_inner())
}

/// Ein einfarbiges JPEG für Tests und den Simulator.
pub fn sample_jpeg(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, QUALITY)
        .encode_image(&img)
        .expect("Testbild ist kodierbar");
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_keep_the_aspect_and_fit_the_edge() {
        let thumb = make(&sample_jpeg(1600, 1200)).unwrap();
        let img = image::load_from_memory(&thumb).unwrap();
        assert_eq!((img.width(), img.height()), (640, 480));
        let small = make(&sample_jpeg(320, 256)).unwrap();
        let img = image::load_from_memory(&small).unwrap();
        assert_eq!((img.width(), img.height()), (320, 256), "nie vergrößern");
    }

    #[test]
    fn broken_files_are_a_readable_error() {
        let err = make(b"kein jpeg").unwrap_err();
        assert!(err.starts_with("Foto nicht lesbar"), "{err}");
    }
}
