use askama::Template;

#[derive(Template)]
#[template(path = "gcontainer.xml")]
pub struct GContainerTemplate {
    pub gain_map_image_len: usize,
}

#[derive(Template)]
#[template(path = "gain_map.xml")]
pub struct HDRGainMapMetadataTemplate {
    pub gain_map_min: f32,
    pub gain_map_max: f32,
    pub gamma: f32,
    pub offset_sdr: f32,
    pub offset_hdr: f32,
    pub hdr_capacity_min: f32,
    pub hdr_capacity_max: f32,
}

pub fn make_xmp(xml: String) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend("http://ns.adobe.com/xap/1.0/\0".as_bytes());
    data.extend(xml.as_bytes());
    data
}

/// Invalid MPF Header, needed in order to first generate the full JPEG to get offset and length info
pub const BOGUS_MPF_HEADER: &[u8] = &[
    b'M', b'P', b'F', 0, // Magic Number
    0x49, 0x49, 0x2A, 0, // Endian Marker (Little here)
    8, 0, 0, 0, // Offset to first IFD (why would that be set to anything else ??)
    // ---- Index IFD
    3, 0, // Count
    // -- Version
    0, 0xB0, // Tag ID (MP Format Version)
    7, 0, // Type (undefined) (NOT in the spec, had to look some other place)
    4, 0, 0, 0, // Count (again, NOT in spec)
    b'0', b'1', b'0', b'0', // Value
    // -- Number of images
    1, 0xB0, // Tag ID (Number of Images)
    4, 0, // Type (Long) (NOT in spec)
    1, 0, 0, 0, // Count (1 long)
    2, 0, 0, 0, // Value
    // -- MP Entry
    2, 0xB0, // Tag ID
    7, 0, // Type (undefined)
    0x20, 0, 0, 0, // Count (16 * number of images = 32)
    0x32, 0, 0, 0, // Offset to MP Entries
    0, 0, 0, 0, // Padding ?
    // ---- MP Entry 1
    0, 0, 3, 0, // Individual Image Attribute
    0, 0, 0,
    0, // Individual Image Size (between SOI and EOI) (dunno what this really refers to)
    0, 0, 0, 0, // Individual Image Data Offset (zero for first image)
    0, 0, // Dependant Image 1 Entry Number
    0, 0, // Dependant Image 2 Entry Number
    // ---- MP Entry 2
    0, 0, 0, 0, // Individual Image Attribute
    0, 0, 0, 0, // Individual Image Size (between SOI and EOI)
    0, 0, 0, 0, // Individual Image Data Offset (relative to endian marker)
    0, 0, // Dependant Image 1 Entry Number
    0, 0, // Dependant Image 2 Entry Number
];
