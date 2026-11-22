// Adapted from SpektraLab negative_hue.metal (b51e672, GPL-3.0-or-later).
// Hue readout from developed negative density, after paper scan and before glare.
struct Params {
    frame: vec4<u32>, // width, height
    gate: vec4<u32>, // left, top, content width, height
    lo: vec4<f32>,
    scale: vec4<f32>,
    neutral: vec4<f32>,
    light: vec4<f32>,
}
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage,read> film: array<f32>;
@group(0) @binding(2) var<storage,read_write> optical: array<f32>;
@group(0) @binding(3) var<storage,read> layers: array<vec2<f32>>;
@group(0) @binding(4) var<storage,read> exposure_map: array<vec2<f32>>;
@group(0) @binding(5) var<storage,read> gamut: array<f32>;

fn read_layer(c:u32, d:f32)->vec2<f32>{
    let f=clamp((d-p.lo[c])*p.scale[c],0.0,1023.0);
    let i=min(u32(f),1022u);
    return mix(layers[i*3u+c],layers[(i+1u)*3u+c],f-f32(i));
}

fn map_at(e:vec2<f32>)->vec2<f32>{
    let f=clamp(e,vec2(0.0),vec2(1.0))*159.0;
    let i=min(vec2<u32>(f),vec2(158u));
    let t=f-vec2<f32>(i);
    let j=i.x*160u+i.y;
    return mix(mix(exposure_map[j],exposure_map[j+1u],t.y),
               mix(exposure_map[j+160u],exposure_map[j+161u],t.y),t.x);
}

fn paper_chroma(l:f32, v:vec2<f32>)->f32{
    let f=clamp((l-p.light.x)*p.light.y,0.0,47.0);
    let i=min(u32(f),46u);
    let angle=atan2(v.y,v.x)/6.283185307;
    let h=fract(angle)*120.0;
    let j=u32(h)%120u;
    let k=(j+1u)%120u;
    return mix(mix(gamut[i*120u+j],gamut[i*120u+k],fract(h)),
               mix(gamut[(i+1u)*120u+j],gamut[(i+1u)*120u+k],fract(h)),f-f32(i));
}

// Matrix columns, using the same rounded sRGB/XYZ convention as the scanner.
const RGB_XYZ=mat3x3<f32>(vec3(.4124,.2126,.0193),vec3(.3576,.7152,.1192),vec3(.1805,.0722,.9505));
const XYZ_RGB=mat3x3<f32>(vec3(3.2406,-.9689,.0557),vec3(-1.5372,1.8758,-.204),vec3(-.4986,.0415,1.057));
const M1=mat3x3<f32>(vec3(.8189330101,.0329845436,.0482003018),vec3(.3618667424,.9293118715,.2643662691),vec3(-.1288597137,.0361456387,.633851707));
const M2=mat3x3<f32>(vec3(.2104542553,1.9779984951,.0259040371),vec3(.793617785,-2.428592205,.7827717662),vec3(-.0040720468,.4505937099,-.808675766));
const M2_INV=mat3x3<f32>(vec3(1.0,1.0,1.0),vec3(.39633779,-.10556134,-.08948418),vec3(.21580376,-.06385417,-1.29148554));
const M1_INV=mat3x3<f32>(vec3(1.22701385,-.04058018,-.07638128),vec3(-.55779998,1.11225687,-.42148198),vec3(.28125615,-.07167668,1.58616322));

@compute @workgroup_size(1024)
fn main(@builtin(global_invocation_id) gid:vec3<u32>){
    let i=gid.x;
    if i>=p.frame.x*p.frame.y{return;}
    let k=i*3u;
    let rgb=vec3(optical[k],optical[k+1u],optical[k+2u]);
    // Match the optical scan on bypassed pixels, including film shoulders.
    if p.frame.z!=0u {
        let bounded=clamp(rgb,vec3(0.0),vec3(1.0));
        optical[k]=bounded.x; optical[k+1u]=bounded.y; optical[k+2u]=bounded.z;
    }
    let pos=vec2<u32>(i%p.frame.x,i/p.frame.x);
    if any(pos<p.gate.xy)||any(pos>=p.gate.xy+p.gate.zw){return;}
    let r=read_layer(0u,film[k]);let g=read_layer(1u,film[k+1u]);let b=read_layer(2u,film[k+2u]);
    let exposure=pow(vec3(10.0),vec3(r.x,g.x,b.x))/p.neutral.xyz;
    let shares=exposure/dot(exposure,vec3(1.0));
    let ab=map_at(shares.xy);
    let lms=M1*(RGB_XYZ*rgb);
    let lab=M2*(sign(lms)*pow(abs(lms),vec3(1.0/3.0)));
    let co=length(lab.yz);let cr=length(ab);
    if co<=1e-7||cr<=1e-9{return;}
    let t=smoothstep(.005,.020,cr*lab.x/.5687);
    let u=.5*dot(shares,vec3(r.y,g.y,b.y));
    let w=(t/(1.0+u*u)-.02)/.98;
    if w<=0.0{return;}
    let dir=mix(lab.yz/co,ab/cr,w);
    if length(dir)<=1e-6{return;}
    let ratio=co/max(paper_chroma(lab.x,dir),.0001);
    var c=co;
    if ratio>.85{let z=(ratio-.85)/.15;c*=mix(1.0,(.85+.15*z/sqrt(1.0+z*z))/ratio,w);}
    let root=M2_INV*vec3(lab.x,c*normalize(dir));
    let result=XYZ_RGB*(M1_INV*(root*root*root));
    optical[k]=result.x;optical[k+1u]=result.y;optical[k+2u]=result.z;
}
