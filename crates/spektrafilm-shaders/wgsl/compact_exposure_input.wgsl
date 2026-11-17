fn exposure_rgb(pixel: u32) -> vec3<f32> {
    let base = pixel * 3u;
    return vec3<f32>(rgb_in[base], rgb_in[base + 1u], rgb_in[base + 2u]);
}
