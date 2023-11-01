use dirs::home_dir;
use minifb::{ Key, Window, WindowOptions };

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
        // .join("Desktop/test.png")
        .to_str().unwrap()
        .to_owned()
}

pub fn render_image(width: usize, height: usize, buffer: &Vec<u32>) {
    let mut window = Window::new(
        "Test - ESC to exit",
        width,
        height,
        WindowOptions::default()
    ).unwrap();
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));
    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update_with_buffer(&buffer, width, height).unwrap();
    }
}

#[inline]
pub fn rgba_to_u32(rgba: &[u8]) -> u32 {
    ((rgba[3] as u32) << 24) | ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | (rgba[2] as u32)
}

