// https://en.wikipedia.org/wiki/SRGB
// There is another definition in the ITU document...
pub fn _srgb_gamma(linear_color: f32) -> f32 {
    if linear_color <= 0.0031308 {
        12.92 * linear_color
    } else {
        1.055 * linear_color.powf(2.4f32.recip()) - 0.055
    }
}

pub fn gamma(linear_color: f32, gamma: f32) -> f32 {
    linear_color.powf(gamma.recip())
}
