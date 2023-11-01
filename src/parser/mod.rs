extern crate crc32fast;

use std::convert::TryInto;
use std::fs::read;
use std::io::Read;
use std::ops::Sub;
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

fn render_image(buffer: &Vec<u32>) {

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

    render_image(&decode_data);

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

impl PngParam {
    fn new(raw_body: &[u8]) -> Self {
        let width = u32::from_be_bytes((&raw_body[0..4]).try_into().unwrap());
        let height = u32::from_be_bytes((&raw_body[4..8]).try_into().unwrap());
        let depth = raw_body[8];
        let color_type = raw_body[9];
        let compression = raw_body[10];
        let filter = raw_body[11];
        let interlace = raw_body[12] == 1;

        PngParam {
            width,
            height,
            depth,
            color_type,
            compression,
            filter,
            interlace
        }
    }
}

struct PhysParam {
    x_pixels_per_unit: u32,
    y_pixels_per_unit: u32,
    unit_specifier: u8,
}

impl PhysParam {
    fn new(raw_body: &[u8]) -> Self {

        let x_pixels_per_unit  = u32::from_be_bytes(raw_body[0..4].try_into().unwrap());
        let y_pixels_per_unit  = u32::from_be_bytes(raw_body[4..8].try_into().unwrap());
        let unit_specifier = raw_body[8];

        Self {
            x_pixels_per_unit,
            y_pixels_per_unit,
            unit_specifier
        }
    }
}

/// 暂时只考虑真彩色图片
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8
}

impl Sub for Pixel {
    type Output = Self;

    fn sub(self, rhs: Self) -> Pixel {
        Pixel {
            r: self.r - rhs.r,
            g: self.g - rhs.g,
            b: self.b - rhs.b,
            a: self.a - rhs.a
        }
    }
}

struct Scanline {
    filter: u8,
    color_channel: usize,
    data: Vec<u8>
}

impl Scanline {

    fn new(bytes: &[u8], color_type: u8) -> Scanline {

        let color_channel = match color_type {
            0 => 1,
            2 => 3,
            3 => 1,
            4 => 2,
            6 => 4,
            _ => panic!("错误的颜色类型")
        };

        if (bytes.len() - 1) % color_channel != 0  {
            panic!("扫描线长度异常");
        }

        // let mut data: Vec<Pixel> = Vec::new();
        // let pixels = (bytes.len() - 1) / color_channel;
        // for pixel in 0..pixels {
        //     let index = pixel * color_channel + 1;
        //     data.push(Pixel {
        //         r: bytes[index],
        //         g: bytes[index + 1],
        //         b: bytes[index + 2],
        //         a: bytes[index + 3]
        //     });
        // }

        Scanline {
            color_channel,
            data: bytes[1..].to_vec(),
            filter: bytes[0],
        }
    }

    fn decode_sub(&self) -> Vec<u32> {

        let mut result: Vec<u32> = Vec::new();

        #[inline]
        fn sub_filter(a: u8, b: u8) -> u8 {
            let c = a as i32 - b as i32;
            if c < 0 {
                255
            } else {
                c as u8
            }
        }

        // [255, 0, 0, 255]
        // [1, 255, 0, 0]

        let mut previous: [u8; 4] = [0, 0, 0, 0];
        let pixels = self.data.len() / self.color_channel;
        for pixel in 0..pixels {
            let index = pixel * self.color_channel;

            previous[0] = sub_filter(self.data[index], previous[0]);
            previous[1] = sub_filter(self.data[index + 1], previous[1]);
            previous[2] = sub_filter(self.data[index + 2], previous[2]);
            previous[3] = sub_filter(self.data[index + 3], previous[3]);

            let decode = rgba_to_u32(&previous);

            if self.filter == 1 {
                println!("{:?}", previous);
            }

            result.push(decode);
        }

        if self.filter == 1 {
            println!("{:?}", result);
        }

        result
    }

}

pub struct PngParser {
    params: Option<PngParam>,
    phys: Option<PhysParam>,
    raw_data: Vec<u8>,
    file_path: String,
    index: usize,
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
            phys: None,
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
                "IHDR" => self.params = Some(PngParam::new(block_data)),
                "iCCP" => {},
                "pHYs" => self.phys = Some(PhysParam::new(block_data)),
                "IDAT" => self.parse_idat_block(block_data),
                "IEND" => self.parse_iend_block(),
                _ => panic!("解析到不支持的 png 块")
            }

        }

    }

    fn parse_iend_block(&mut self) {

    }

    fn parse_idat_block(&self, raw_body: &[u8]) {

        let mut decoder = ZlibDecoder::new(raw_body);
        let mut decode_data = Vec::new();
        decoder.read_to_end(&mut decode_data).expect("解压 IDAT 块数据失败");

        let Some(params) = &self.params else {
            panic!("Not params");
        };

        let scanline_vec: Vec<u32> = decode_data
            .chunks((1 + 4 * params.width) as usize)
            .map(|chunk| Scanline::new(chunk, params.color_type).decode_sub())
            .flatten()
            .collect();

        let demo = scanline_vec[0..398].to_owned();
        let mut rr = Vec::new();
        for _ in 0..398 {
            rr.append(&mut demo.clone());
        }

        render_image(&rr);

        // for item in scanline_vec {
        //     // println!("{}", item.filter);
        // }

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

