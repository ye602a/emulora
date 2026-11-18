// Optional source-anchored stochastic coordinates for film grain.
// Zero-initialized reference preserves the original per-render RNG byte for byte.
struct GrainReference { row_x:vec4<f32>, row_y:vec4<f32>, shape:vec4<u32> }
fn grain_position(i:u32, width:u32, r:GrainReference)->vec2<f32> {
    let p=vec2<f32>(f32(i%width),f32(i/width));
    if(r.shape.w==0u){return p;}
    let edge=vec3<f32>(p+vec2<f32>(0.5),1.0);
    let source=vec2<f32>(dot(r.row_x.xyz,edge),dot(r.row_y.xyz,edge))-vec2<f32>(0.5);
    if(r.shape.w==2u){return source;}
    return clamp(source,vec2<f32>(0.0),vec2<f32>(r.shape.yz-vec2<u32>(1u)));
}
fn grain_cell_index(cell:vec2<i32>,stride:u32,r:GrainReference)->u32 {
    let index=bitcast<vec2<u32>>(cell);
    if(r.shape.w==2u && any(index>=r.shape.yz)){
        // Signed coordinates outside the original raster have their own seeds.
        return 0x80000000u ^ (index.x*0x85ebca6bu + index.y*0xc2b2ae35u);
    }
    return index.y*stride+index.x;
}
fn grain_index(i:u32,r:GrainReference)->u32 {
    if(r.shape.w==0u){return i;}
    let p=vec2<i32>(floor(grain_position(i,r.shape.x,r)+vec2<f32>(0.5)));
    return grain_cell_index(p,r.shape.y,r);
}
