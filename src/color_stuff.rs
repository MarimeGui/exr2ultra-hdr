// https://github.com/MONOGRID/gainmap-js
// https://helpx.adobe.com/content/dam/help/en/camera-raw/using/gain-map/jcr_content/root/content/flex/items/position/position-par/table/row-io13dug-column-4a63daf/download_section/download-1/Gain_Map_1_0d14.pdf
// https://developer.android.com/media/platform/hdr-image-format
// https://openexr.com/en/latest/TechnicalIntroduction.html#
// https://stackoverflow.com/questions/45605506/how-are-cie-xyy-luminance-values-for-color-primaries-determined

// http://www.brucelindbloom.com/index.html?Eqn_XYZ_to_xyY.html

use exr::math::Vec2;

use crate::{Matrix3x1f, Matrix3x3f};

// ----- Pixel

/// Linear-light pixel
#[derive(Default, Copy, Clone, Debug)]
pub struct Pixel {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl From<Matrix3x1f> for Pixel {
    fn from(value: Matrix3x1f) -> Self {
        Self {
            r: value[(0, 0)],
            g: value[(1, 0)],
            b: value[(2, 0)],
        }
    }
}

impl From<Pixel> for Matrix3x1f {
    fn from(value: Pixel) -> Self {
        Self::new(value.r, value.g, value.b)
    }
}

// ----- CIE xy coords

/// xy CIE 1391 coordinates
#[derive(Copy, Clone, Debug)]
pub struct CIExyCoords {
    pub x: f32,
    pub y: f32,
}

impl CIExyCoords {
    /// Effectively add luma and turn these coordinates into xyY
    pub fn with_luma(self, luma: f32) -> CIExyYCoords {
        CIExyYCoords { coords: self, luma }
    }

    // https://en.wikipedia.org/wiki/Standard_illuminant
    pub fn from_black_body(temperature: f32) -> CIExyCoords {
        // Not handling below 4000K and above 25000K properly but oh well
        let x = if temperature <= 7000.0 {
            0.244063
                + 0.09911 * 10.0f32.powi(3) * temperature.recip()
                + 2.9678 * 10.0f32.powi(6) * temperature.powi(2).recip()
                - 4.6070 * 10.0f32.powi(9) * temperature.powi(3).recip()
        } else {
            0.237040
                + 0.24748 * 10.0f32.powi(3) * temperature.recip()
                + 1.9018 * 10.0f32.powi(6) * temperature.powi(2).recip()
                - 2.0064 * 10.0f32.powi(9) * temperature.powi(3).recip()
        };
        let y = -3.000 * x.powi(2) + 2.870 * x - 0.275;

        CIExyCoords { x, y }
    }

    pub fn has_negatives(&self) -> bool {
        self.x.is_sign_negative() | self.y.is_sign_negative()
    }
}

impl From<Vec2<f32>> for CIExyCoords {
    fn from(value: Vec2<f32>) -> Self {
        Self {
            x: value.0,
            y: value.1,
        }
    }
}

// ----- CIE XYZ coords

#[derive(Copy, Clone, Debug)]
pub struct CIEXYZCoords {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl CIEXYZCoords {
    // http://www.brucelindbloom.com/index.html?Eqn_XYZ_to_xyY.html
    /// Takes in XYZ coordinates and returns xyY
    pub fn to_xyy(self, illuminant: CIExyCoords) -> CIExyYCoords {
        // Handle pure black
        if (self.x < f32::EPSILON) & (self.y < f32::EPSILON) & (self.z < f32::EPSILON) {
            // If pure black, return white point with zero luma
            return illuminant.with_luma(0.0);
        }

        CIExyYCoords {
            coords: CIExyCoords {
                x: self.x / (self.x + self.y + self.z),
                y: self.y / (self.x + self.y + self.z),
            },
            luma: self.y,
        }
    }
}

impl From<Matrix3x1f> for CIEXYZCoords {
    fn from(value: Matrix3x1f) -> Self {
        Self {
            x: value[(0, 0)],
            y: value[(1, 0)],
            z: value[(2, 0)],
        }
    }
}

impl From<CIEXYZCoords> for Matrix3x1f {
    fn from(value: CIEXYZCoords) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<CIExyYCoords> for CIEXYZCoords {
    fn from(value: CIExyYCoords) -> Self {
        // Black
        if value.luma < f32::EPSILON {
            return Self {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
        }

        Self {
            x: (value.coords.x * value.luma) / value.coords.y,
            y: value.luma,
            z: ((1.0 - value.coords.x - value.coords.y) * value.luma) / value.coords.y,
        }
    }
}

// ----- CIE xyY coords

/// CIE xyY coordinates, x and y refer to the color, Y is luma
#[derive(Copy, Clone, Debug)]
pub struct CIExyYCoords {
    pub coords: CIExyCoords,
    pub luma: f32,
}

// ----- Chromaticities

/// Use to define a color space
#[derive(Copy, Clone, Debug)]
pub struct Chromaticities {
    pub red: CIExyCoords,
    pub green: CIExyCoords,
    pub blue: CIExyCoords,
    pub white: CIExyCoords,
}

impl From<exr::meta::attribute::Chromaticities> for Chromaticities {
    fn from(value: exr::meta::attribute::Chromaticities) -> Self {
        Self {
            red: value.red.into(),
            green: value.green.into(),
            blue: value.blue.into(),
            white: value.white.into(),
        }
    }
}

impl From<Chromaticities> for png::SourceChromaticities {
    fn from(value: Chromaticities) -> Self {
        Self::new(
            (value.white.x, value.white.y),
            (value.red.x, value.red.y),
            (value.green.x, value.green.y),
            (value.blue.x, value.blue.y),
        )
    }
}

impl From<png::SourceChromaticities> for Chromaticities {
    fn from(value: png::SourceChromaticities) -> Self {
        Self {
            red: CIExyCoords {
                x: value.red.0.into_value(),
                y: value.red.1.into_value(),
            },
            green: CIExyCoords {
                x: value.green.0.into_value(),
                y: value.green.1.into_value(),
            },
            blue: CIExyCoords {
                x: value.blue.0.into_value(),
                y: value.blue.1.into_value(),
            },
            white: CIExyCoords {
                x: value.white.0.into_value(),
                y: value.white.1.into_value(),
            },
        }
    }
}

impl Chromaticities {
    // http://www.brucelindbloom.com/index.html?Eqn_RGB_XYZ_Matrix.html
    /// Use this matrix to go from RGB values to CIE XYZ values. This matrix goes first in multiplication order
    pub fn rgb_to_xyz_matrix(&self) -> Option<Matrix3x3f> {
        let red: CIEXYZCoords = self.red.with_luma(1.0).into();
        let green: CIEXYZCoords = self.green.with_luma(1.0).into();
        let blue: CIEXYZCoords = self.blue.with_luma(1.0).into();
        let white: CIEXYZCoords = self.white.with_luma(1.0).into();

        let s_coefficients = Matrix3x3f::new(
            red.x, green.x, blue.x, red.y, green.y, blue.y, red.z, green.z, blue.z,
        )
        .try_inverse()?
            * Matrix3x1f::from(white);
        let s_r = s_coefficients[(0, 0)];
        let s_g = s_coefficients[(1, 0)];
        let s_b = s_coefficients[(2, 0)];

        Some(Matrix3x3f::new(
            s_r * red.x,
            s_g * green.x,
            s_b * blue.x,
            s_r * red.y,
            s_g * green.y,
            s_b * blue.y,
            s_r * red.z,
            s_g * green.z,
            s_b * blue.z,
        ))
    }

    pub fn xyz_to_rgb_matrix(&self) -> Option<Matrix3x3f> {
        self.rgb_to_xyz_matrix()?.try_inverse()
    }

    /// Matrix for going from this color space to another one. If destination space is smaller than this one, be careful of output. This matrix comes first in multiplication
    pub fn rgb_space_conversion_matrix(&self, destination: &Chromaticities) -> Option<Matrix3x3f> {
        Some(destination.xyz_to_rgb_matrix()? * self.rgb_to_xyz_matrix()?)
    }

    /// Does this color space contain this color ?
    pub fn contains_color(&self, color: CIExyCoords) -> bool {
        // https://stackoverflow.com/a/2049593
        fn sign(p1: CIExyCoords, p2: CIExyCoords, p3: CIExyCoords) -> f32 {
            (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
        }

        let d1 = sign(color, self.red, self.green);
        let d2 = sign(color, self.green, self.blue);
        let d3 = sign(color, self.blue, self.red);

        let has_neg = (d1 < 0.0) | (d2 < 0.0) | (d3 < 0.0);
        let has_pos = (d1 > 0.0) | (d2 > 0.0) | (d3 > 0.0);

        !(has_neg & has_pos)
    }

    /// Does this color space cover another color space completely ? Does not take into account white point
    pub fn contains_space(&self, other: &Chromaticities) -> bool {
        self.contains_color(other.red)
            & self.contains_color(other.green)
            & self.contains_color(other.blue)
    }

    /// Use to calculate the luminance of a pixel
    pub fn luminance_values(&self) -> Option<LuminanceCoefficients> {
        let mat = self.rgb_to_xyz_matrix()?;

        Some(LuminanceCoefficients {
            red: mat[(1, 0)],
            green: mat[(1, 1)],
            blue: mat[(1, 2)],
        })
    }

    /// True if any component is negative
    pub fn has_negatives(&self) -> bool {
        self.red.has_negatives()
            | self.green.has_negatives()
            | self.blue.has_negatives()
            | self.white.has_negatives()
    }
}

// ----- Luminance coefficients

/// Use to calculate the luminance of an RGB pixel
#[derive(Debug)]
pub struct LuminanceCoefficients {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}
