use clap::ValueEnum;

use crate::color_stuff::{CIExyCoords, Chromaticities};

// -----

pub const D50_ILLUMINANT: CIExyCoords = CIExyCoords {
    x: 0.34567,
    y: 0.35850,
};

// https://en.wikipedia.org/wiki/Standard_illuminant#Illuminant_series_D
// There are more precise definitions (Wikipedia), but using official ITU values used in Rec. 709 and 2020
pub const D65_ILLUMINANT: CIExyCoords = CIExyCoords {
    x: 0.3127,
    y: 0.3290,
};

pub const ACES_ILLUMINANT: CIExyCoords = CIExyCoords {
    x: 0.32168,
    y: 0.33767,
};

// -----

#[derive(ValueEnum, Debug, Copy, Clone)]
pub enum ColorSpace {
    Rec709,
    Rec2020,
    Rec2100,
    AcesAp0,
    AcesAp1,
    DisplayP3,
}

impl ColorSpace {
    pub fn chromaticities(&self) -> Chromaticities {
        match self {
            ColorSpace::Rec709 => REC_709,
            ColorSpace::Rec2020 => REC_2020,
            ColorSpace::Rec2100 => REC_2100,
            ColorSpace::AcesAp0 => ACES_AP0,
            ColorSpace::AcesAp1 => ACES_AP1,
            ColorSpace::DisplayP3 => DISPLAY_P3,
        }
    }
}

// https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.709-6-201506-I!!PDF-E.pdf
pub const REC_709: Chromaticities = Chromaticities {
    red: CIExyCoords { x: 0.640, y: 0.330 },
    green: CIExyCoords { x: 0.300, y: 0.600 },
    blue: CIExyCoords { x: 0.150, y: 0.060 },
    white: D65_ILLUMINANT,
};

// https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2020-0-201208-S!!PDF-E.pdf
pub const REC_2020: Chromaticities = Chromaticities {
    red: CIExyCoords { x: 0.708, y: 0.292 },
    green: CIExyCoords { x: 0.170, y: 0.797 },
    blue: CIExyCoords { x: 0.131, y: 0.046 },
    white: D65_ILLUMINANT,
};

// https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2100-2-201807-I!!PDF-E.pdf
pub const REC_2100: Chromaticities = REC_2020;

// https://en.wikipedia.org/wiki/Academy_Color_Encoding_System
pub const ACES_AP0: Chromaticities = Chromaticities {
    red: CIExyCoords {
        x: 0.7347,
        y: 0.2653,
    },
    green: CIExyCoords { x: 0.0, y: 1.0 },
    blue: CIExyCoords {
        x: 0.0001,
        y: -0.0770,
    },
    white: ACES_ILLUMINANT,
};

// https://en.wikipedia.org/wiki/Academy_Color_Encoding_System
pub const ACES_AP1: Chromaticities = Chromaticities {
    red: CIExyCoords { x: 0.713, y: 0.293 },
    green: CIExyCoords { x: 0.165, y: 0.830 },
    blue: CIExyCoords { x: 0.128, y: 0.044 },
    white: ACES_ILLUMINANT,
};

// https://en.wikipedia.org/wiki/DCI-P3
pub const DISPLAY_P3: Chromaticities = Chromaticities {
    red: CIExyCoords { x: 0.680, y: 0.320 },
    green: CIExyCoords { x: 0.265, y: 0.690 },
    blue: CIExyCoords { x: 0.150, y: 0.060 },
    white: D65_ILLUMINANT,
};
