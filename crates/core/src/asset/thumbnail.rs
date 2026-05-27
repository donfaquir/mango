use std::path::Path;

use image::imageops::FilterType;

pub const THUMB_SIZE: u32 = 200;

/// Generate a 200x200 webp thumbnail at `dest` from the image at `source`,
/// and return the source image's original (width, height).
///
/// Cover-mode (`resize_to_fill`) keeps every thumbnail the same physical size
/// so grid layouts line up; Lanczos3 is the best quality/speed tradeoff at
/// this scale.
pub fn generate_with_metadata(
    source: &Path,
    dest: &Path,
) -> image::ImageResult<(u32, u32)> {
    let img = image::open(source)?;
    let dims = (img.width(), img.height());
    let resized = img.resize_to_fill(THUMB_SIZE, THUMB_SIZE, FilterType::Lanczos3);
    resized.save_with_format(dest, image::ImageFormat::WebP)?;
    Ok(dims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};
    use tempfile::tempdir;

    fn write_sample_png(path: &Path, w: u32, h: u32) {
        let buf: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(w, h, |x, _| Rgb([(x % 256) as u8, 0, 0]));
        buf.save(path).unwrap();
    }

    #[test]
    fn thumbnail_size_is_200x200() {
        let td = tempdir().unwrap();
        let src = td.path().join("src.png");
        let dst = td.path().join("thumb.webp");
        write_sample_png(&src, 640, 480);

        let dims = generate_with_metadata(&src, &dst).unwrap();
        assert_eq!(dims, (640, 480));

        let thumb = image::open(&dst).unwrap();
        assert_eq!(thumb.width(), THUMB_SIZE);
        assert_eq!(thumb.height(), THUMB_SIZE);
    }

    #[test]
    fn thumbnail_format_is_webp() {
        let td = tempdir().unwrap();
        let src = td.path().join("src.png");
        let dst = td.path().join("thumb.webp");
        write_sample_png(&src, 100, 100);

        generate_with_metadata(&src, &dst).unwrap();
        let bytes = std::fs::read(&dst).unwrap();
        // RIFF....WEBP magic
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WEBP");
    }

    #[test]
    fn returns_error_for_invalid_image() {
        let td = tempdir().unwrap();
        let src = td.path().join("not_an_image.png");
        std::fs::write(&src, b"definitely not an image").unwrap();
        let dst = td.path().join("thumb.webp");
        assert!(generate_with_metadata(&src, &dst).is_err());
    }
}
