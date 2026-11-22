// OFX colour-negative bleach bypass. XYZ are dye densities; W is neutral silver density.
fn negative_bleach_density(cmy: vec3<f32>, amount: f32, cyan_max: f32, cyan_coupling: f32) -> vec4<f32> {
    let d = max(cmy, vec3<f32>(0.0));
    let layers = d / (d + vec3<f32>(0.85));
    let weight = amount * (layers.r + layers.g + layers.b) / 3.0;
    var dyes = mix(cmy, vec3<f32>(max(cmy.r, max(cmy.g, cmy.b))), weight);
    dyes.r = max(dyes.r - amount * 0.30 * cyan_coupling * clamp(d.r / cyan_max, 0.0, 1.0), 0.0);
    return vec4<f32>(dyes, 0.22 * weight);
}
