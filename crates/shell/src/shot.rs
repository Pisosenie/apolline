use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use iced::window::Screenshot;

pub fn save(path: &Path, shot: &Screenshot) -> Result<(), Box<dyn Error>> {
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, shot.size.width, shot.size.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&shot.rgba)?;
    Ok(())
}
