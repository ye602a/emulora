// Matches the resolved FilmBorderGpuParams uniform.
struct FilmBorderParams {
    canvas: vec2<u32>,
    _pad0: vec2<u32>,
    content: vec4<u32>,
    origin_px: vec2<f32>,
    gate_half_mm: vec2<f32>,
    along: vec2<f32>,
    across: vec2<f32>,
    strip_half_mm: f32,
    edge_exposure: f32,
    perforation_pitch_mm: f32,
    perforation_corner_mm: f32,
    perforation_half_mm: vec2<f32>,
    perforation_row_mm: f32,
    _pad1: f32,
}

fn film_raster_pixel(pixel: u32) -> vec2<u32> {
    return vec2<u32>(pixel % film_border.canvas.x, pixel / film_border.canvas.x);
}

fn film_mm(pixel: vec2<u32>) -> vec2<f32> {
    let centered = vec2<f32>(pixel) + vec2<f32>(0.5) - film_border.origin_px;
    return vec2<f32>(dot(film_border.along, centered), dot(film_border.across, centered));
}

fn film_present(position: vec2<f32>) -> bool {
    let t = abs(position.y);
    if t < film_border.gate_half_mm.y {
        return true;
    }
    if t >= film_border.strip_half_mm {
        return false;
    }
    let pitch = film_border.perforation_pitch_mm;
    let s = position.x - pitch * floor(position.x / pitch + 0.5);
    let hole = abs(vec2<f32>(s, t - film_border.perforation_row_mm));
    let corner = film_border.perforation_corner_mm;
    let q = max(hole - film_border.perforation_half_mm + vec2<f32>(corner), vec2<f32>(0.0));
    return dot(q, q) > corner * corner;
}

fn film_coverage(pixel: u32) -> f32 {
    return select(0.0, 1.0, film_present(film_mm(film_raster_pixel(pixel))));
}
