extern crate crc32fast;

use std::convert::TryInto;
use std::fs::read;
use std::io::Read;
use std::ops::Sub;

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
            c.abs() as u8
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

            // if self.filter == 1 {
            //     println!("{:?}", previous);
            // }

            result.push(decode);
        }

        // if self.filter == 1 {
        //     println!("{:?}", result);
        // }

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

        println!("{:?}", &self.params);

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

        render_image(398, 398, &rr);

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

}

