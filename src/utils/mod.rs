use dirs::home_dir;

pub fn is_grayscale(color_type: u8) -> bool {
    color_type == 0
}

pub fn is_true_color(color_type: u8) -> bool {
    color_type == 2
}

pub fn is_indexed_color(color_type: u8) -> bool {
    color_type == 3
}

pub fn is_grayscale_with_alpha(color_type: u8) -> bool {
    color_type == 4
}

pub fn is_true_color_with_alpha(color_type: u8) -> bool {
    color_type == 6
}

pub fn u8_4_to_usize(bytes: &[u8]) -> usize {
    u32::from_be_bytes(bytes.try_into().unwrap()) as usize
}

pub fn get_png_dir() -> String {
    home_dir().unwrap()
        .join("Desktop/pixels-large.png")
        .to_str().unwrap()
        .to_owned()
}

