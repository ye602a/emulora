fn exposure_rgb(pixel: u32) -> vec3<f32> {
    let raster = film_raster_pixel(pixel);
    let position = film_mm(raster);
    let origin = film_border.content.xy;
    let extent = film_border.content.zw;
    if all(abs(position) < film_border.gate_half_mm)
        && all(raster >= origin) && all(raster < origin + extent)
    {
        let source = raster - origin;
        let base = (source.y * extent.x + source.x) * 3u;
        return vec3<f32>(rgb_in[base], rgb_in[base + 1u], rgb_in[base + 2u]);
    }
    return vec3<f32>(select(0.0, film_border.edge_exposure, film_present(position)));
}
