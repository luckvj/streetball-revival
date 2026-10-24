//! RGBA8 to PNG.

/// `rgba` (straight alpha, top row first) as a PNG file. Fully opaque
/// images are written as RGB.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let opaque = rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255);
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width.max(1), height.max(1));
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let data: Vec<u8> = if width == 0 || height == 0 {
        encoder.set_color(png::ColorType::Rgba);
        vec![0; 4]
    } else if opaque {
        encoder.set_color(png::ColorType::Rgb);
        rgba.as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect()
    } else {
        encoder.set_color(png::ColorType::Rgba);
        rgba.to_vec()
    };
    let mut writer = encoder.write_header().expect("PNG header");
    writer.write_image_data(&data).expect("PNG data");
    writer.finish().expect("PNG end");
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_a_png() {
        let png = super::encode(1, 1, &[1, 2, 3, 4]);
        assert_eq!(&png[1..4], b"PNG");
    }
}
