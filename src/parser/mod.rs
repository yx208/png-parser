extern crate crc32fast;

use std::convert::TryInto;
use std::fs::read;
use std::io::Read;
use std::usize;

use flate2::read::ZlibDecoder;
use minifb::{ Key, Window, WindowOptions };
use super::utils::{
    get_png_dir,
    u8_4_to_usize
};

static WIDTH: usize = 398;
static HEIGHT: usize = 398;

#[inline]
fn rgba_to_u32(rgba: &[u8]) -> u32 {
    ((rgba[3] as u32) << 24) | ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | (rgba[2] as u32)
}

///
/// Sub 过滤函数
///
fn decode_sub_filter(row: &[u8], prev_row: &[u8; 4]) -> [u8; 4] {
    // println!("{:?}/{:?}", prev_row, row);
    let mut result = [0, 0, 0, 0];
    for i in 0..row.len() {
        let v = (row[i] as i32 - prev_row[i] as i32).abs();
        result[i] = v as u8;
    }
    result
}

fn render_image(buffer: Vec<u32>) {

    // let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new(
        "Test - ESC to exit",
        WIDTH,
        HEIGHT,
        WindowOptions::default()
    ).unwrap();
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));
    while window.is_open() && !window.is_key_down(Key::Escape) {
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

fn parse_scan_line(line: &Vec<u8>) -> Vec<u32> {
    let (filter_type, data) = line.split_at(1);
    let filter_type = filter_type[0];
    let temp: [u8; 4] = [0, 0, 0, 0];
    data
        .chunks(4)
        .into_iter()
        .scan(temp, |acc, chunk| {
            *acc = match filter_type {
                1..=4 => decode_sub_filter(chunk, acc),
                _ => [0, 0, 0, 0]
            };
            Some(rgba_to_u32(acc))
        })
        .collect()
}

fn parse_idat_block(block: &Vec<u8>) -> Result<(), ()> {

    let mut decoder = ZlibDecoder::new(&block[..]);
    let mut decode_data = Vec::new();
    decoder.read_to_end(&mut decode_data).unwrap();

    // 把数组转成以一根扫面线为一个 Vec 的 Vec
    let decode_data: Vec<Vec<u8>> = decode_data
        .chunks(1 + 4 * 398)
        .map(|chunk| chunk.to_vec())
        .collect();

    println!("{:?}", &decode_data[1]);

    // 迭代每一根扫描线，进行解析，返回一个集合，这个集合是解析扫描线转换后的 Vec<u32>
    let decode_data: Vec<u32> = decode_data
        .into_iter()
        .map(|line| parse_scan_line(&line))
        .into_iter()
        .flatten()
        .collect();

    render_image(decode_data);

    // let (_, data) = decode_data[0].split_at(1);
    // let r: Vec<Vec<u8>> = data.chunks(4).map(|x| x.to_vec()).collect();
    // println!("{:?}", &r);
    //
    // // println!("{:?}", &data[0]);

    Ok(())
}

pub fn run() {

    // let png_data = std::fs::read(get_png_dir()).unwrap();
    // let (_, png_body) = png_data.split_at(8);
    // let mut iter = png_body.iter();
    // parse_block(&mut iter);

}

struct PngParam {
    width: u32,
    height: u32,
    depth: u8,
    color_type: u8,
    compression: u8,
    filter: u8,
    interlace: bool
}

pub struct PngParser {
    params: Option<PngParam>,
    raw_data: Vec<u8>,
    file_path: String,
    index: usize
}

impl PngParser {

    pub fn new(png_path: String) -> Self {

        let png_sign: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
        let mut png_data = read(&png_path).expect("无法读取图片");

        if !png_data.starts_with(&png_sign) {
            panic!("并不是 png 图片");
        }

        png_data.drain(0..8);

        Self {
            params: None,
            raw_data: png_data,
            file_path: png_path.to_owned(),
            index: 0,
        }
    }

    pub fn parse(&mut self) {

        let raw_data_size = self.raw_data.len();

        if raw_data_size < 8 {
            panic!("解析图片数据块头部出错");
        }

        while self.index < raw_data_size {

            // 解析出块头部
            let block_size = &self.raw_data[self.index..self.index + 4];
            let block_type = &self.raw_data[self.index + 4..self.index + 8];
            let block_size = u8_4_to_usize(block_size);
            self.index += 8;

            // 当前已索引大小 + 当前块内容需要的大小 + 4 字节的 crc
            if raw_data_size < (self.index + block_size + 4) {
                panic!("解析图片数据块内容出错");
            }

            // 解析出块内容
            let offset_end_size = self.index + block_size;
            let block_data = &self.raw_data[self.index..offset_end_size];
            let block_crc = u32::from_be_bytes(
                (&self.raw_data[offset_end_size..offset_end_size + 4])
                    .try_into()
                    .unwrap()
            );
            self.index = offset_end_size + 4;

            // 检查 crc
            let mut hasher = crc32fast::Hasher::new();
            hasher.update(block_type);
            hasher.update(block_data);
            if block_crc != hasher.finalize() {
                panic!("CRC 校验失败");
            }

            // 解析具体块内容
            let block_type = unsafe { std::str::from_utf8_unchecked(block_type) };
            match block_type {
                "IHDR" => {

                }
                "iCCP" => {

                }
                "pHYs" => {

                }
                "IDAT" => {

                }
                "IEND" => {

                }
                _ => {}
            }

        }

    }

}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn should_be_run() {
        let png_path = get_png_dir();
        let mut parser = PngParser::new(png_path);
        parser.parse();
    }

    #[test]
    fn test_parse_u32() {
        let a = [255, 0, 0, 255];
        let b = [0, 255, 255, 0];
        let c = decode_sub_filter(&a, &b);
        println!("{}", rgba_to_u32(&c));
    }

}

