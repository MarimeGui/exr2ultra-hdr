use std::{
    fs::File,
    io::{BufWriter, Cursor, Write},
    path::PathBuf,
};

use askama::Template;
use clap::Parser;
use exr::image::read::{image::ReadLayers, layers::ReadChannels, read};
use jpeg_encoder::Encoder as JPEGEncoder;
use nalgebra::SMatrix;
use png::{Encoder as PNGEncoder, ScaledFloat};
use rcms::IccProfile;

use color_spaces::{ColorSpace, Illuminant, REC_709};
use color_stuff::{Chromaticities, LuminanceCoefficients, Pixel};
use transfer_functions::gamma as gamma_transfer;
use ultra_hdr_stuff::{make_xmp, GContainerTemplate, HDRGainMapMetadataTemplate, BOGUS_MPF_HEADER};

mod color_spaces;
mod color_stuff;
mod transfer_functions;
mod ultra_hdr_stuff;

// ----- Constants

const GAMMA: f32 = 2.4;
const JPEG_QUALITY: u8 = 100;
/// Gain Map SDR offset
const OFFSET_SDR: f32 = 1.0 / 64.0;
/// Gain Map HDR offset
const OFFSET_HDR: f32 = 1.0 / 64.0;
/// Gamma value used for encoding Gain Map to JPEG
const MAP_GAMMA: f32 = 1.0;
/// JPEG Quality of Gain Map
const MAP_JPEG_QUALITY: u8 = 100;

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

    let write_chromaticities = output_chromaticities.unwrap_or(input_chromaticities);

    // Get multiplication factor
    let factor = if let Some(ev) = args.exposure {
        2.0f32.powf(ev)
    } else {
        1.0
    };

    // Apply transfer function and limit to 1.0 (convert to display-referred) and convert to u8, all while calculating gain map
    let mut image_data = Vec::with_capacity(width * height);
    let mut pixel_gains = Vec::with_capacity(width * height);
    let coefficients = write_chromaticities.luminance_values().unwrap();
    for pixel in linear_light {
        pixel_gains.push(calculate_gain(
            &pixel,
            factor,
            &coefficients,
            OFFSET_HDR,
            OFFSET_SDR,
        ));

        let r = process_pixel(pixel.r, factor, GAMMA);
        let g = process_pixel(pixel.g, factor, GAMMA);
        let b = process_pixel(pixel.b, factor, GAMMA);
        image_data.extend([r, g, b])
    }

    // Compute encoded gain map, as specified in Google documentation
    let min_content_boost = pixel_gains
        .iter()
        .min_by(|x, y| x.partial_cmp(y).unwrap())
        .unwrap();
    let max_content_boost = pixel_gains
        .iter()
        .max_by(|x, y| x.partial_cmp(y).unwrap())
        .unwrap();
    let map_min_log2 = min_content_boost.log2();
    let map_max_log2 = max_content_boost.log2();
    let mut encoded_recoveries = Vec::with_capacity(width * height);
    for pixel_gain in pixel_gains {
        let log_recovery = (pixel_gain.log2() - map_min_log2) / (map_max_log2 - map_min_log2);
        let clamped_recovery = log_recovery.clamp(0.0, 1.0);
        let recovery = clamped_recovery.powf(MAP_GAMMA);
        encoded_recoveries.push((recovery * 255.0).round() as u8)
    }

    // Write PNG image
    if let Some(png_path) = args.png {
        encode_png(png_path, &image_data, width, height, write_chromaticities)
    }

    // Write JPEG image
    if let Some(jpg_path) = args.jpg {
        // TODO: Implement MPF
        // Might have to use https://crates.io/crates/img-parts to modify offset

        // Create new file
        let mut write_file = BufWriter::new(File::create(jpg_path).unwrap());

        // Gen Gain Map XMP data
        let hdr_xmp = HDRGainMapMetadataTemplate {
            gain_map_min: map_min_log2,
            gain_map_max: map_max_log2,
            gamma: MAP_GAMMA,
            offset_sdr: OFFSET_SDR,
            offset_hdr: OFFSET_HDR,
            hdr_capacity_min: map_min_log2,
            hdr_capacity_max: map_max_log2,
        }
        .render()
        .unwrap();

        // Encode gain map image
        let mut gain_map_image_bytes = Cursor::new(Vec::new());
        let mut gain_map_encoder = JPEGEncoder::new(&mut gain_map_image_bytes, MAP_JPEG_QUALITY);
        gain_map_encoder
            .add_app_segment(1, &make_xmp(hdr_xmp))
            .unwrap();
        gain_map_encoder
            .encode(
                &encoded_recoveries,
                width.try_into().unwrap(),
                height.try_into().unwrap(),
                jpeg_encoder::ColorType::Luma,
            )
            .unwrap();
        let gain_map_image_bytes = gain_map_image_bytes.into_inner();

        // Gen directory XMP
        let directory_xmp = GContainerTemplate {
            gain_map_image_len: gain_map_image_bytes.len(),
        }
        .render()
        .unwrap();

        // Generate ICC profile
        let mut profile_bytes = Cursor::new(Vec::new());
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
        profile.serialize(&mut profile_bytes).unwrap();

        // Encode main image
        let mut main_encoder = JPEGEncoder::new(&mut write_file, JPEG_QUALITY);
        main_encoder
            .add_icc_profile(&profile_bytes.into_inner())
            .unwrap();
        main_encoder
            .add_app_segment(1, &make_xmp(directory_xmp))
            .unwrap();
        // Add wrong MPF header, file still works in Chrome though
        main_encoder.add_app_segment(2, BOGUS_MPF_HEADER).unwrap();
        main_encoder
            .encode(
                &image_data,
                width.try_into().unwrap(),
                height.try_into().unwrap(),
                jpeg_encoder::ColorType::Rgb,
            )
            .unwrap();

        // Put gain map image next
        write_file.write_all(&gain_map_image_bytes).unwrap()
    }
}

/// Compute gain value for this pixel, used to build gain map for Ultra HDR JPEG
fn calculate_gain(
    pixel: &Pixel,
    factor: f32,
    coefficients: &LuminanceCoefficients,
    offset_hdr: f32,
    offset_sdr: f32,
) -> f32 {
    let hdr_luminance =
        pixel.r * coefficients.red + pixel.g * coefficients.green + pixel.b * coefficients.blue;

    let sdr_pixel = Pixel {
        r: (pixel.r * factor).clamp(0.0, 1.0),
        g: (pixel.g * factor).clamp(0.0, 1.0),
        b: (pixel.b * factor).clamp(0.0, 1.0),
    };

    let sdr_luminance = sdr_pixel.r * coefficients.red
        + sdr_pixel.g * coefficients.green
        + sdr_pixel.b * coefficients.blue;

    (hdr_luminance + offset_hdr) / (sdr_luminance + offset_sdr)
}

/// Go from scene-referred linear light value to scene-referred gamma-encoded u8 pixel component
fn process_pixel(linear_value: f32, factor: f32, gamma: f32) -> u8 {
    (gamma_transfer(linear_value * factor, gamma) * 255.0)
        .clamp(0.0, 255.0)
        .round() as u8
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
