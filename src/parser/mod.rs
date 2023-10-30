extern crate crc32fast;

use std::convert::TryInto;
use std::io::Read;

use dirs::home_dir;
use flate2::read::ZlibDecoder;
use minifb::{ Key, Window, WindowOptions };

static WIDTH: usize = 398;
static HEIGHT: usize = 398;

fn render_image() {

    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new(
        "Test - ESC to exit",
        WIDTH,
        HEIGHT,
        WindowOptions::default()
    ).unwrap();

    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            *i = 0;
        }
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
    }

}

#[inline]
fn task_iter(iter: &mut dyn Iterator<Item = &u8>, size: usize) -> Vec<u8> {
    iter.take(size).cloned().collect::<Vec<u8>>()
}

fn parse_block(iter: &mut dyn Iterator<Item = &u8>) {

    println!("块大小 \t 块类型 \t crc");

    loop {

        // 取出块头部信息
        let block_head = task_iter(iter, 8);
        let block_size = u32::from_be_bytes(block_head[0..4].try_into().unwrap());
        let block_type = String::from_utf8(block_head[4..8].try_into().unwrap()).unwrap();
        let block_body = task_iter(iter, block_size as usize);
        let block_crc = u32::from_be_bytes(task_iter(iter, 4).try_into().unwrap());

        println!("{block_size} \t {block_type} \t\t {block_crc}");

        // 检查 crc
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(block_type.as_bytes());
        hasher.update(&block_body);

        assert_eq!(block_crc, hasher.finalize());
        match block_type.as_str() {
            "IHDR" => parse_ihdr_block(&block_body).unwrap(),
            "iCCP" => parse_iccp_block(&block_body).unwrap(),
            "pHYs" => parse_phys_block(&block_body).unwrap(),
            "IDAT" => parse_idat_block(&block_body).unwrap(),
            "IEND" => break,
            _ => ()
        }

    }

    // render_image();
}

fn parse_iccp_block(block: &Vec<u8>) -> Result<(), ()> {

    let index = block.iter().position(|&x| x == 0u8).unwrap() + 1;
    let part1 = &block[..index];
    let part2 = &block[index + 1..];
    let _name = String::from_utf8(part1.try_into().unwrap()).unwrap();

    let mut icc_data = Vec::new();
    let mut zlib_decoder = ZlibDecoder::new(part2);
    zlib_decoder.read_to_end(&mut icc_data).unwrap();
    let _data = lcms2::Profile::new_icc(&icc_data).unwrap();

    Ok(())
}

fn parse_ihdr_block(block: &Vec<u8>) -> Result<(), ()> {

    let width = u32::from_be_bytes(block[0..4].try_into().unwrap());
    let height = u32::from_be_bytes(block[4..8].try_into().unwrap());
    let depth = block.get(8).unwrap().to_owned();
    let color_type = block.get(9).unwrap().to_owned();
    let _compression = block.get(10).unwrap().to_owned();
    let filter = block.get(11).unwrap().to_owned();
    let interlace = block.get(12).unwrap().to_owned();

    println!("宽度：{width} \t 高度：{height} \t 通道深度：{depth} \t 色彩类型：{color_type} \t 过滤类型：{filter} \t 交错：{interlace}");

    Ok(())
}

fn parse_phys_block(block: &Vec<u8>) -> Result<(), ()> {

    let _x_pixels_per_unit  = u32::from_be_bytes(block[0..4].try_into().unwrap());
    let _y_pixels_per_unit  = u32::from_be_bytes(block[4..8].try_into().unwrap());
    let _unit_specifier = block.get(8).unwrap();

    Ok(())
}

fn parse_idat_block(block: &Vec<u8>) -> Result<(), ()> {

    let mut decoder = ZlibDecoder::new(&block[..]);
    let mut decode_data = Vec::new();
    decoder.read_to_end(&mut decode_data).unwrap();

    let data = decode_data
        .chunks(398 * 4 + 1)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<u8>>>();

    println!("{:?}", &data[0]);

    Ok(())
}

fn get_png_dir() -> String {
    home_dir().unwrap()
        .join("Desktop/pixels-large.png")
        .to_str().unwrap()
        .to_owned()
}

pub fn run() {

    let png_data = std::fs::read(get_png_dir()).unwrap();
    let (_, png_body) = png_data.split_at(8);
    let mut iter = png_body.iter();
    parse_block(&mut iter);

}

