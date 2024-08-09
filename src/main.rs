use std::{fs::File, io::BufWriter, path::PathBuf};

use clap::Parser;
use color_stuff::Pixel;
use exr::image::read::{image::ReadLayers, layers::ReadChannels, read};
use nalgebra::SMatrix;
use png::{Encoder, ScaledFloat};

use color_spaces::{ColorSpace, REC_709};
use transfer_functions::gamma;

mod color_spaces;
mod color_stuff;
mod transfer_functions;

// ----- Matrix type definitions

type Matrix3x1f = SMatrix<f32, 3, 1>;
type Matrix3x3f = SMatrix<f32, 3, 3>;

// -----

#[derive(Parser)]
struct App {
    /// Manually specify what the linear-light RGB channels refer to
    #[arg(short, long)]
    input_chromaticities: Option<ColorSpace>,
    /// What the output will be encoded in. If not specified, will be the same as input
    #[arg(short, long)]
    output_chromaticities: Option<ColorSpace>,
    /// Path to scene-referred linear-light OpenEXR image
    exr: PathBuf,
    /// Path to display-referred gamma-encoded PNG image
    png: PathBuf,
}

// -----

fn main() {
    let args = App::parse();

    let image = read()
        .no_deep_data()
        .largest_resolution_level()
        .all_channels()
        .first_valid_layer()
        .all_attributes()
        .from_file(args.exr)
        .unwrap();

    // Get chromaticities
    let input_chromaticities = if let Some(c) = image.attributes.chromaticities {
        c.into()
    } else if let Some(c) = args.input_chromaticities {
        c.chromaticities()
    } else {
        eprintln!("Warning: Assuming Rec. 709 (sRGB) color space for input EXR.");
        REC_709
    };

    // Load pixels to own vec
    let width = image.attributes.display_window.size.0;
    let height = image.attributes.display_window.size.1;
    let mut linear_light = vec![Pixel::default(); width * height];
    for channel in image.layer_data.channel_data.list {
        for (index, sample) in channel.sample_data.values_as_f32().enumerate() {
            if channel.name.to_string() == "R" {
                linear_light[index].r = sample;
            } else if channel.name.to_string() == "G" {
                linear_light[index].g = sample;
            } else if channel.name.to_string() == "B" {
                linear_light[index].b = sample;
            }
        }
    }

    // Convert to desired color space
    if let Some(output_chromaticities) = args.output_chromaticities {
        let conversion_matrix = input_chromaticities
            .rgb_space_conversion_matrix(&output_chromaticities.chromaticities());
        for pixel in &mut linear_light {
            let v: Matrix3x1f = (*pixel).into();
            *pixel = (conversion_matrix * v).into()
        }
    }

    // Apply transfer function and limit to 1.0 (convert to display-referred), convert to u8
    let mut image_data = Vec::with_capacity(width * height);
    for pixel in linear_light {
        let r = (gamma(pixel.r, 2.4) * 255.0).clamp(0.0, 255.0).round() as u8;
        let g = (gamma(pixel.g, 2.4) * 255.0).clamp(0.0, 255.0).round() as u8;
        let b = (gamma(pixel.b, 2.4) * 255.0).clamp(0.0, 255.0).round() as u8;
        image_data.extend([r, g, b])
    }

    // Write to image
    let mut encoder = Encoder::new(
        BufWriter::new(File::create(args.png).unwrap()),
        width.try_into().unwrap(),
        height.try_into().unwrap(),
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_gamma(ScaledFloat::new(2.4f32.recip()));
    encoder.set_source_chromaticities(
        args.output_chromaticities
            .map(|c| c.chromaticities())
            .unwrap_or(input_chromaticities)
            .into(),
    );
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&image_data).unwrap()
}

// TODO: Option to convert from Color space to another
// TODO: How to embed an ICC profile with my own chromaticities ?
