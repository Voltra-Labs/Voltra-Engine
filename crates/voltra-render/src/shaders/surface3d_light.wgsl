// The surface prelude's lighting: Filament's BRDF — GGX, height-correlated
// Smith, Schlick, Lambert diffuse — under one directional light and a
// uniform ambient, and the unlit alternative. Follows surface3d.wgsl
// (`shader::SURFACE_3D`); `render3d::surface::brdf` is the CPU twin.

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
// `s` lit by the frame's light and ambient, as the standard block's
// `unlit` says: Filament's BRDF, capped at MAX_LIGHT.
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

    // The directional light.
    let l = frame.light_direction.xyz;
    let h = unit_or(v + l, n);
    let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
    let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
    let l_dot_h = clamp(dot(l, h), 0.0, 1.0);
    let specular = d_ggx(n_dot_h, cross(n, h), a) * v_smith_ggx_correlated(n_dot_v, n_dot_l, a)
        * f_schlick(f0, l_dot_h);
    let direct = (diffuse_color * (1.0 / PI) + specular) * frame.light_color.rgb * n_dot_l
        * frame.light_direction.w;

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
