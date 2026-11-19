// Applied after camera optics and before film development, on existing stock only.
fn add_latent_exposure(pixel: u32) {
    let pos = film_mm(film_raster_pixel(pixel));
    if !film_present(pos) { return; }
    var extra = vec3<f32>(0.0);
    if film_border.leak_shape.z > 0.0 {
        let d = (pos - film_border.leak_shape.xy) * film_border.leak_shape.zw;
        extra = exp(-0.5 * dot(d, d)) * film_border.leak_gain.xyz;
    }
    let cell = floor((pos - film_border.imprint_shape.xy) * film_border.imprint_shape.z);
    if film_border.imprint_shape.z > 0.0 && all(cell >= vec2<f32>(0.0)) && cell.x < 64.0 && cell.y < 5.0 {
        let x = u32(cell.x);
        let index = x / 4u;
        let glyph = film_border.glyphs[index / 4u][index % 4u];
        if x % 4u < 3u {
            let bit = 14u - u32(cell.y) * 3u - x % 4u;
            if ((glyph >> bit) & 1u) != 0u { extra += film_border.imprint_gain.xyz; }
        }
    }
    let base = 3u * pixel;
    data[base] += extra.x;
    data[base + 1u] += extra.y;
    data[base + 2u] += extra.z;
}
