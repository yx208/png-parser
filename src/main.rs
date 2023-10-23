extern crate crc32fast;

use std::convert::TryInto;
use std::fs::read;
use std::io::Read;

use dirs::home_dir;
use flate2::read::ZlibDecoder;
use lcms2;

fn main() {

    let png_file = read(get_png_dir()).unwrap();

    let (_, data) = png_file.split_at(8);
    let mut data_iter = data.iter();
    parse_block(&mut data_iter);

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
            "IDHR" => parse_ihdr_block(&block_body).unwrap(),
            "iCCP" => parse_iccp_block(&block_body).unwrap(),
            "pHYs" => parse_phys_block(&block_body).unwrap(),
            "IDAT" => parse_iat_block(&block_body).unwrap(),
            "IEND" => break,
            _ => ()
        }

    }
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
    let _width = u32::from_be_bytes(block[0..4].try_into().unwrap());
    let _height = u32::from_be_bytes(block[4..8].try_into().unwrap());
    let _depth = block.get(8).unwrap().to_owned();
    let _color_type = block.get(9).unwrap().to_owned();
    let _compression = block.get(10).unwrap().to_owned();
    let _filter = block.get(11).unwrap().to_owned();
    let _interlace = block.get(12).unwrap().to_owned();
    Ok(())
}

fn parse_phys_block(block: &Vec<u8>) -> Result<(), ()> {
    let _x_pixels_per_unit  = u32::from_be_bytes(block[0..4].try_into().unwrap());
    let _y_pixels_per_unit  = u32::from_be_bytes(block[4..8].try_into().unwrap());
    let _unit_specifier = block.get(8).unwrap();
    Ok(())
}

fn parse_iat_block(block: &Vec<u8>) -> Result<(), ()> {

    println!("{:?}", block);

    Ok(())
}

fn get_png_dir() -> String {
    home_dir().unwrap()
        .join("Desktop/pixels-large.png")
        .to_str().unwrap()
        .to_owned()
}
