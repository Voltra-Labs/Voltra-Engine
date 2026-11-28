// The surface prelude's lighting: Filament's BRDF — GGX, height-correlated
// Smith, Schlick, Lambert diffuse — under the frame's directional lights,
// the point and spot lights of the instance's list (Filament's windowed
// inverse square and cone) and a uniform ambient, and the unlit
// alternative. Follows surface3d.wgsl (`shader::SURFACE_3D`);
// `render3d::surface::brdf` and `render3d::lighting` are the CPU twins.

// A directional light: toward it (unit length), its colour times energy.
struct DirectionalGpu {
    toward: vec4<f32>,
    color: vec4<f32>,
};

// A point or spot light (`lighting::LightsUniform3D`): xyz where it is, w
// 1 / range²; rgb its colour times energy, w the cone's scale; xyz the way
// a spot shines, w the cone's offset — a point light's cone is scale 0,
// offset 1, lighting every way.
struct PunctualGpu {
    position: vec4<f32>,
    color: vec4<f32>,
    spot: vec4<f32>,
};

// The frame's lights. counts.x directional, counts.y point and spot.
struct Lights3D {
    counts: vec4<u32>,
    directional: array<DirectionalGpu, 4>,
    punctual: array<PunctualGpu, 128>,
};

@group(0) @binding(1) var<uniform> lights: Lights3D;

// A light a centimetre across: the inverse square stays finite on it.
const MIN_DISTANCE_SQUARED: f32 = 1.0e-4;

// GGX / Trowbridge-Reitz, Filament's form: 1 − (n·h)² from |n × h|².
fn d_ggx(n_dot_h: f32, n_cross_h: vec3<f32>, a: f32) -> f32 {
    let one_minus = dot(n_cross_h, n_cross_h);
    let ah = n_dot_h * a;
    let k = a / (one_minus + ah * ah);
    return k * k * (1.0 / PI);
}

// Height-correlated Smith, the visibility V = G / (4 n·l n·v).
fn v_smith_ggx_correlated(n_dot_v: f32, n_dot_l: f32, a: f32) -> f32 {
    let a2 = a * a;
    let ggx_v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let ggx_l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / (ggx_v + ggx_l);
}

fn f_schlick(f0: vec3<f32>, l_dot_h: f32) -> vec3<f32> {
    let f = pow(1.0 - l_dot_h, 5.0);
    return f + f0 * (1.0 - f);
}

// The split-sum's DFG term for a uniform environment, Karis's analytic fit
// ("Physically Based Shading on Mobile"): what a constant sky reflects.
fn env_brdf(f0: vec3<f32>, roughness: f32, n_dot_v: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}
// One light's share at a point: `radiance` (its colour times energy and
// falloff) arriving from `l`, through the BRDF.
fn direct_light(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, radiance: vec3<f32>, diffuse_color: vec3<f32>, f0: vec3<f32>, a: f32, n_dot_v: f32) -> vec3<f32> {
    let h = unit_or(v + l, n);
    let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
    let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
    let l_dot_h = clamp(dot(l, h), 0.0, 1.0);
    let specular = d_ggx(n_dot_h, cross(n, h), a) * v_smith_ggx_correlated(n_dot_v, n_dot_l, a)
        * f_schlick(f0, l_dot_h);
    return (diffuse_color * (1.0 / PI) + specular) * radiance * n_dot_l;
}

// What a point or spot light gives a point `to_light` from it, per unit of
// its colour: Filament's — the inverse square, windowed smoothly to 0 at
// the range by (1 − (d²/r²)²)², times the cone's saturate(cos · scale +
// offset)². `PunctualLight::attenuation` on the CPU.
fn punctual_attenuation(light: PunctualGpu, to_light: vec3<f32>) -> f32 {
    let d2 = dot(to_light, to_light);
    let factor = d2 * light.position.w;
    let window = clamp(1.0 - factor * factor, 0.0, 1.0);
    let from_light = -unit_or(to_light, vec3<f32>(0.0));
    let ramp = clamp(dot(light.spot.xyz, from_light) * light.color.w + light.spot.w, 0.0, 1.0);
    return window * window / max(d2, MIN_DISTANCE_SQUARED) * ramp * ramp;
}

// The `i`th of the instance's point and spot light indices; 255 is none.
fn light_index(list: vec3<u32>, i: u32) -> u32 {
    let word = select(list.y, list.x, i < 4u);
    return (word >> ((i % 4u) * 8u)) & 0xFFu;
}

// `s` lit by the frame's lights — the directional ones whose bit the
// instance's list sets, the point and spot lights it names — and ambient,
// as the standard block's `unlit` says: Filament's BRDF, capped at
// MAX_LIGHT. The loops have fixed bounds and no texture in them, so they
// leave the uniformity of what follows alone.
fn light_surface(s: Surface, in: SurfaceInput) -> vec3<f32> {
    let base = s.base_color;
    let metallic = clamp(s.metallic, 0.0, 1.0);
    let roughness = clamp(s.roughness, MIN_ROUGHNESS, 1.0);
    let occlusion = clamp(s.occlusion, 0.0, 1.0);
    let n = unit_or(s.normal, in.normal);
    let v = in.view;

    let diffuse_color = base * (1.0 - metallic);
    let f0 = mix(vec3<f32>(0.04), base, metallic);
    let n_dot_v = min(abs(dot(n, v)) + 1.0e-5, 1.0);
    let a = roughness * roughness;

    var direct = vec3<f32>(0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        if i < lights.counts.x && ((in.light_list.z >> i) & 1u) == 1u {
            let sun = lights.directional[i];
            direct = direct + direct_light(n, v, sun.toward.xyz, sun.color.rgb, diffuse_color, f0, a, n_dot_v);
        }
    }
    for (var i = 0u; i < 8u; i = i + 1u) {
        let index = light_index(in.light_list, i);
        if index < lights.counts.y {
            let light = lights.punctual[index];
            let to_light = light.position.xyz - in.world;
            let radiance = light.color.rgb * punctual_attenuation(light, to_light);
            direct = direct + direct_light(n, v, unit_or(to_light, n), radiance, diffuse_color, f0, a, n_dot_v);
        }
    }

    // The ambient: a uniform sky, specular (the split sum's E = f0·A + B)
    // and diffuse (its irradiance π·L times albedo / π) taking what the
    // specular leaves, (1 − E) — Filament's — occluded.
    let reflected = env_brdf(f0, roughness, n_dot_v);
    let ambient = (diffuse_color * (vec3<f32>(1.0) - reflected) + reflected) * frame.ambient.rgb
        * occlusion;

    let lit = min(direct + ambient + max(s.emissive, vec3<f32>(0.0)), vec3<f32>(MAX_LIGHT));
    return select(lit, unlit_surface(s), standard.flags.y > 0.5);
}

// `s` drawn as it is: its base colour, no light.
fn unlit_surface(s: Surface) -> vec3<f32> {
    return clamp(s.base_color, vec3<f32>(0.0), vec3<f32>(MAX_LIGHT));
}
