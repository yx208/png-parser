extern crate crc32fast;

use std::convert::TryInto;
use std::fs::read;
use std::io::Read;

use flate2::read::ZlibDecoder;

use super::utils::{
    get_png_dir,
    u8_4_to_usize,
    rgba_to_u32,
    render_image,
};

#[derive(Debug)]
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
#[derive(Clone)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8
}

impl Pixel {

    fn sub(&self, rhs: &Pixel) -> Pixel {
        Pixel {
            r: self.sub_filter(self.r, rhs.r),
            g: self.sub_filter(self.g, rhs.g),
            b: self.sub_filter(self.b, rhs.b),
            a: self.sub_filter(self.a, rhs.a)
        }
    }

    fn add(&self, rhs: &Pixel) -> Pixel {
        Pixel {
            r: self.sub_filter(self.r, rhs.r),
            g: self.sub_filter(self.g, rhs.g),
            b: self.sub_filter(self.b, rhs.b),
            a: self.sub_filter(self.a, rhs.a)
        }
    }

    fn to_u32(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | (self.a as u32)
    }

    #[inline]
    fn sub_filter(&self, a: u8, b: u8) -> u8 {
        a.wrapping_sub(b)
        // ((a as i32) - (b as i32)).abs() as u8
    }

    #[inline]
    fn add_filter(&self, a: u8, b: u8) -> u8 {
        a.wrapping_add(b)
        // ((a as i32) - (b as i32)).abs() as u8
    }
}

struct Scanline {
    filter: u8,
    color_channel: usize,
    data: Vec<Pixel>
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

        let mut data: Vec<Pixel> = Vec::new();
        let pixels = (bytes.len() - 1) / color_channel;
        for pixel in 0..pixels {
            let index = pixel * color_channel + 1;
            data.push(Pixel {
                r: bytes[index],
                g: bytes[index + 1],
                b: bytes[index + 2],
                a: bytes[index + 3]
            });
        }

        Scanline {
            data,
            color_channel,
            filter: bytes[0],
        }
    }

    fn decode_sub(&self) -> Vec<Pixel> {
        let mut previous = Pixel { r: 0, g: 0, b: 0, a: 0 };
        let result: Vec<Pixel> = self.data.iter()
            .map(|pixel| {
                previous = pixel.sub(&previous);
                previous.clone()
            })
            .collect();

        result
    }

    fn decode_up(&self, previous: &Vec<Pixel>) -> Vec<Pixel> {
        self.data.iter().enumerate().map(|(index, pixel)| {
            pixel.sub(previous.get(index).unwrap())
        }).collect()
    }

    fn decode_paeth(&self, previous: &Vec<Pixel>) -> Vec<Pixel> {
        let mut temp = Pixel { r: 0, g: 0, b: 0, a: 0 };
        let result: Vec<Pixel> = self.data.iter()
            .map(|pixel| {
                temp = pixel.sub(&temp);
                temp.clone()
            })
            .collect();

        result
    }

    fn paeth_predictor(&self, a: &Pixel, b: &Pixel, c: &Pixel) {
        let p = a.add(b).sub(c);
        let pa = p.sub(a);
        let pb = p.sub(b);
        let pc = p.sub(c);
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
                "sRGB" => {},
                "iDOT" => {},
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

        let mut scanline_list: Vec<Scanline> = decode_data
            .chunks((1 + 4 * params.width) as usize)
            .map(|chunk| Scanline::new(chunk, params.color_type))
            .collect();

        let mut result: Vec<Vec<Pixel>> = Vec::new();
        for (index, scanline) in scanline_list.iter_mut().enumerate() {
            match scanline.filter {
                1 => {
                    result.push(scanline.decode_sub());
                },
                2 => {
                    let line = scanline.decode_up(result.get_mut(index - 1).unwrap());
                    result.push(line);
                }
                4 => {
                    let line = scanline.decode_paeth(result.get_mut(index - 1).unwrap());
                    result.push(line);
                }
                _ => {}
            };
        }

        // let scanline_vec: Vec<u32> = decode_data
        //     .chunks((1 + 4 * params.width) as usize)
        //     .map(|chunk| Scanline::new(chunk, params.color_type).decode_sub())
        //     .flatten()
        //     .collect();
        //
        // let demo = scanline_vec[0..398].to_owned();
        // let mut rr = Vec::new();
        // for _ in 0..398 {
        //     rr.append(&mut demo.clone());
        // }
        //
        // render_image(398, 398, &rr);

        // for item in scanline_vec {
        //     // println!("{}", item.filter);
        // }

    }

    fn map_color_type_to_channel(color_type: u8) -> usize {
         match color_type {
            0 => 1,
            2 => 3,
            3 => 1,
            4 => 2,
            6 => 4,
            _ => panic!("错误的颜色类型")
        }
    }

    fn decode_sub(&self, pixel_data: &[u8]) -> Vec<u32> {

        let Some(params) = &(self.params) else { panic!(""); };
        let channel = PngParser::map_color_type_to_channel(params.color_type);
        let mut result: Vec<u32> = Vec::new();

        #[inline]
        fn sub_filter(a: u8, b: u8) -> u8 {
            let c = a as i32 - b as i32;
            c.abs() as u8
        }

        let mut previous: [u8; 4] = [0, 0, 0, 0];
        let pixels = pixel_data.len() / channel;
        for pixel in 0..pixels {
            let index = pixel * channel;

            previous[0] = sub_filter(pixel_data[index], previous[0]);
            previous[1] = sub_filter(pixel_data[index + 1], previous[1]);
            previous[2] = sub_filter(pixel_data[index + 2], previous[2]);
            previous[3] = sub_filter(pixel_data[index + 3], previous[3]);

            let decode = rgba_to_u32(&previous);

            result.push(decode);
        }

        result
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

}

