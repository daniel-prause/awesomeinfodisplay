pub fn convert_to_gray_scale(bytes: &Vec<u8>) -> Vec<u8> {
    let mut buffer = Vec::new();
    for chunk in bytes.chunks(6) {
        let gray = (0.299 * i32::pow(chunk[0] as i32, 2) as f32
            + 0.587 * i32::pow(chunk[1] as i32, 2) as f32
            + 0.114 * i32::pow(chunk[2] as i32, 2) as f32)
            .sqrt();
        let gray2 = (0.299 * i32::pow(chunk[3] as i32, 2) as f32
            + 0.587 * i32::pow(chunk[4] as i32, 2) as f32
            + 0.114 * i32::pow(chunk[5] as i32, 2) as f32)
            .sqrt();
        buffer.push(((gray / 16.0) as u8) << 4 | ((gray2 / 16.0) as u8));
    }
    buffer
}

pub fn adjust_brightness_rgb(bytes: &Vec<u8>, brightness: f32) -> Vec<u8> {
    let mut converted_sb_rgb = Vec::with_capacity(49152);
    let set_brightness = |chunk_param: u8| (chunk_param as f32 * brightness as f32 / 100.0) as u8;

    for chunk in bytes.chunks(3) {
        let chunk_two = set_brightness(chunk[2]);
        let chunk_one = set_brightness(chunk[1]);
        let chunk_zero = set_brightness(chunk[0]);
        // for display
        converted_sb_rgb.push(chunk_two);
        converted_sb_rgb.push(chunk_one);
        converted_sb_rgb.push(chunk_zero);
    }

    return converted_sb_rgb;
}

pub fn rgb_bytes_to_rgba_image(bytes: &Vec<u8>, width: u32, height: u32) -> iced::widget::Image {
    let mut converted_sb_rgba = Vec::with_capacity(width as usize * height as usize * 3usize);
    // build rgba for preview
    for chunk in bytes.chunks(3) {
        converted_sb_rgba.append(&mut vec![chunk[2], chunk[1], chunk[0], 255]);
    }

    return iced::widget::Image::new(iced::widget::image::Handle::from_pixels(
        width,
        height,
        converted_sb_rgba,
    ));
}

pub fn swap_rgb(bytes: &Vec<u8>, width: u32, height: u32) -> Vec<u8> {
    let mut swapped = Vec::with_capacity(width as usize * height as usize * 3usize);
    // build rgba for preview
    for chunk in bytes.chunks(3) {
        swapped.append(&mut vec![chunk[2], chunk[1], chunk[0]]);
    }
    return swapped;
}
