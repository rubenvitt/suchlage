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

/// Wie [`sample_jpeg`], aber mit EXIF samt GPS-Position (52° 58′ N), wie es eine Drohne schreibt.
#[cfg(test)]
pub fn sample_jpeg_with_gps(width: u32, height: u32) -> Vec<u8> {
    // TIFF (Little Endian): IFD0 mit Verweis auf das GPS-IFD, darin Breitengrad und Halbkugel.
    let mut tiff: Vec<u8> = b"II\x2a\x00\x08\x00\x00\x00".to_vec();
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&[0x25, 0x88, 4, 0, 1, 0, 0, 0, 26, 0, 0, 0]);
    tiff.extend_from_slice(&0u32.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&[1, 0, 2, 0, 2, 0, 0, 0, b'N', 0, 0, 0]);
    tiff.extend_from_slice(&[2, 0, 5, 0, 3, 0, 0, 0, 56, 0, 0, 0]);
    tiff.extend_from_slice(&0u32.to_le_bytes());
    for (num, den) in [(52u32, 1u32), (58, 1), (0, 1)] {
        tiff.extend_from_slice(&num.to_le_bytes());
        tiff.extend_from_slice(&den.to_le_bytes());
    }
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    let plain = sample_jpeg(width, height);
    let mut out = plain[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&u16::try_from(app1.len() + 2).unwrap().to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&plain[2..]);
    out
}

/// Trägt die Datei einen EXIF-Block (APP1 mit `Exif\0\0`)?
#[cfg(test)]
pub fn has_exif(jpeg: &[u8]) -> bool {
    jpeg.windows(6).any(|w| w == b"Exif\0\0")
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
    fn thumbnails_drop_exif_and_the_gps_position() {
        let original = sample_jpeg_with_gps(1600, 1200);
        assert!(has_exif(&original), "Testbild trägt EXIF");
        image::load_from_memory(&original).expect("Testbild ist lesbar");
        let thumb = make(&original).unwrap();
        assert!(!has_exif(&thumb), "Vorschaubild ohne EXIF");
        let small = make(&sample_jpeg_with_gps(320, 256)).unwrap();
        assert!(!has_exif(&small), "auch ohne Verkleinern neu kodiert");
    }

    #[test]
    fn broken_files_are_a_readable_error() {
        let err = make(b"kein jpeg").unwrap_err();
        assert!(err.starts_with("Foto nicht lesbar"), "{err}");
    }
}
