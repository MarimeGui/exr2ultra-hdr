use std::{
    fs::File,
    io::{BufWriter, Cursor},
    path::PathBuf,
};

use clap::Parser;
use exr::image::read::{image::ReadLayers, layers::ReadChannels, read};
use jpeg_encoder::Encoder;
use nalgebra::SMatrix;
use png::{Encoder as PNGEncoder, ScaledFloat};
use rcms::IccProfile;

use color_spaces::{ColorSpace, Illuminant, REC_709};
use color_stuff::{Chromaticities, Pixel};
use transfer_functions::gamma as gamma_transfer;

mod color_spaces;
mod color_stuff;
mod transfer_functions;

// ----- Constants

const GAMMA: f32 = 2.4;
const JPEG_QUALITY: u8 = 100;

// ----- Matrix type definitions

type Matrix3x1f = SMatrix<f32, 3, 1>;
type Matrix3x3f = SMatrix<f32, 3, 3>;

// -----

#[derive(Parser)]
struct App {
    /// Manually specify what the linear-light RGB channels refer to
    #[arg(short, long)]
    input_chromaticities: Option<ColorSpace>,
    /// Manually override the input white point
    #[arg(long)]
    input_white: Option<Illuminant>,
    /// Re-expose the shot by specifying an exposition value (eV)
    #[arg(short, long, allow_hyphen_values = true)]
    exposure: Option<f32>,
    /// What the output will be encoded in. If not specified, will be the same as input
    #[arg(short, long)]
    output_chromaticities: Option<ColorSpace>,
    /// Manually override the output white point
    #[arg(long)]
    output_white: Option<Illuminant>,
    /// Write display-referred gamma-encoded output to a PNG file
    #[arg(long)]
    png: Option<PathBuf>,
    /// Write display-referred gamma-encoded output to a JPEG file, with ICC profile embedded
    #[arg(long)]
    jpg: Option<PathBuf>,
    /// Path to scene-referred linear-light OpenEXR image
    exr: PathBuf,
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

    // Get input chromaticities
    let mut input_chromaticities = if let Some(c) = args.input_chromaticities {
        c.chromaticities()
    } else if let Some(c) = image.attributes.chromaticities {
        c.into()
    } else {
        eprintln!("Warning: Assuming Rec. 709 (sRGB) color space for input EXR.");
        REC_709
    };

    // Override input white point
    if let Some(i) = args.input_white {
        input_chromaticities.white = i.white();
    }

    // Get output chromaticities
    let mut output_chromaticities = args.output_chromaticities.map(|c| c.chromaticities());

    // Override output white point
    if let Some(i) = args.output_white {
        if let Some(ch) = &mut output_chromaticities {
            ch.white = i.white();
        } else {
            // Take input chromaticities and change white point, this will lead to a conversion
            let mut modified = input_chromaticities;
            modified.white = i.white();
            output_chromaticities = Some(modified)
        }
    }

    // Get multiplication factor
    let factor = if let Some(ev) = args.exposure {
        2.0f32.powf(ev)
    } else {
        1.0
    };

    // Load pixels to own vec
    let width = image.attributes.display_window.size.0;
    let height = image.attributes.display_window.size.1;
    let mut linear_light = vec![Pixel::default(); width * height];
    for channel in image.layer_data.channel_data.list {
        for (index, sample) in channel.sample_data.values_as_f32().enumerate() {
            if channel.name.to_string() == "R" {
                linear_light[index].r = sample * factor;
            } else if channel.name.to_string() == "G" {
                linear_light[index].g = sample * factor;
            } else if channel.name.to_string() == "B" {
                linear_light[index].b = sample * factor;
            }
        }
    }

    // Convert to desired color space
    if let Some(output_chromaticities) = output_chromaticities {
        if !output_chromaticities.contains_space(&input_chromaticities) {
            eprintln!("Warning: Output color space is smaller than input, check output for any artifacts.")
        }

        let conversion_matrix = input_chromaticities
            .rgb_space_conversion_matrix(&output_chromaticities)
            .unwrap();
        for pixel in &mut linear_light {
            let v: Matrix3x1f = (*pixel).into();
            *pixel = (conversion_matrix * v).into()
        }
    }

    // Apply transfer function and limit to 1.0 (convert to display-referred), convert to u8
    let mut image_data = Vec::with_capacity(width * height);
    for pixel in linear_light {
        let r = (gamma_transfer(pixel.r, GAMMA) * 255.0)
            .clamp(0.0, 255.0)
            .round() as u8;
        let g = (gamma_transfer(pixel.g, GAMMA) * 255.0)
            .clamp(0.0, 255.0)
            .round() as u8;
        let b = (gamma_transfer(pixel.b, GAMMA) * 255.0)
            .clamp(0.0, 255.0)
            .round() as u8;
        image_data.extend([r, g, b])
    }

    let write_chromaticities = output_chromaticities.unwrap_or(input_chromaticities);

    // Write PNG image
    if let Some(png_path) = args.png {
        encode_png(png_path, &image_data, width, height, write_chromaticities)
    }

    // Write JPEG image
    if let Some(jpg_path) = args.jpg {
        encode_jpeg(jpg_path, &image_data, width, height, write_chromaticities)
    }
}

fn encode_png(
    png_path: PathBuf,
    image_data: &[u8],
    width: usize,
    height: usize,
    write_chromaticities: Chromaticities,
) {
    let mut encoder = PNGEncoder::new(
        BufWriter::new(File::create(png_path).unwrap()),
        width.try_into().unwrap(),
        height.try_into().unwrap(),
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_gamma(ScaledFloat::new(GAMMA.recip()));
    if write_chromaticities.has_negatives() {
        eprint!("Warning: Some output chromaticities have negative values, PNGs clamps these to 0. Color WILL be affected.")
    }
    encoder.set_source_chromaticities(write_chromaticities.into());
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(image_data).unwrap();
}

fn encode_jpeg(
    jpg_path: PathBuf,
    image_data: &[u8],
    width: usize,
    height: usize,
    write_chromaticities: Chromaticities,
) {
    // Generate ICC profile
    let mut profile_cursor = Cursor::new(Vec::new());
    let profile = IccProfile::new_rgb(
        write_chromaticities.white.with_luma(1.0).into(),
        (
            write_chromaticities.red.with_luma(1.0).into(),
            write_chromaticities.green.with_luma(1.0).into(),
            write_chromaticities.blue.with_luma(1.0).into(),
        ),
        GAMMA.into(),
    )
    .unwrap();
    profile.serialize(&mut profile_cursor).unwrap();

    // Encode image
    let mut encoder = Encoder::new_file(jpg_path, JPEG_QUALITY).unwrap();
    encoder
        .add_icc_profile(&profile_cursor.into_inner())
        .unwrap();
    encoder
        .encode(
            image_data,
            width.try_into().unwrap(),
            height.try_into().unwrap(),
            jpeg_encoder::ColorType::Rgb,
        )
        .unwrap();
}
