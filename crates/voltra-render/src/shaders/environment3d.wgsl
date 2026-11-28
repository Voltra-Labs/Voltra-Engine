// What surrounds the 3D view: the sky by elevation, and the fog between the
// eye and a point — `lighting::environment` on the CPU, number for number.
// Follows frame3d.wgsl.

// The sky along `direction` (unit length): the gradient at its elevation,
// linear in the angle — the top straight up, the horizon level, the bottom
// straight down.
fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    let elevation = asin(clamp(direction.y, -1.0, 1.0)) * (2.0 / PI);
    let up = mix(frame.sky_horizon.rgb, frame.sky_top.rgb, clamp(elevation, 0.0, 1.0));
    let down = mix(frame.sky_horizon.rgb, frame.sky_bottom.rgb, clamp(-elevation, 0.0, 1.0));
    return select(down, up, elevation >= 0.0);
}

// The way from the eye to `world`: from the eye in perspective, the view's
// own in an orthographic one.
fn eye_ray(world: vec3<f32>) -> vec3<f32> {
    let ray = world - frame.eye.xyz;
    let length_squared = dot(ray, ray);
    let toward = select(frame.forward.xyz, ray * inverseSqrt(max(length_squared, 1.0e-30)), length_squared > 1.0e-30);
    return select(frame.forward.xyz, toward, frame.eye.w > 0.5);
}

// How much of what is at `world` the fog hides: Filament's height fog, a
// density of `density · e^(−falloff · (y − height))` integrated exactly
// from the eye, `1 − e^(−∫)`. Falloff 0 is plain distance fog.
fn fog_amount(world: vec3<f32>) -> f32 {
    let density = frame.fog.x;
    let falloff = frame.fog.z;
    let ray = world - frame.eye.xyz;
    let distance = length(ray);
    let at_eye = density * exp(clamp(-falloff * (frame.eye.y - frame.fog.y), -80.0, 80.0));
    let k = clamp(falloff * ray.y, -80.0, 80.0);
    let along = select((1.0 - exp(-k)) / k, 1.0, abs(k) < 1.0e-4);
    let optical = min(at_eye * along, 1.0e30) * distance;
    return clamp(1.0 - exp(-optical), 0.0, 1.0);
}

// `color`, at `world`, seen through the fog: faded toward the fog's colour,
// tinted by the sky the eye looks toward.
fn apply_fog(color: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let tint = mix(frame.fog_color.rgb, sky_color(eye_ray(world)), frame.fog_color.w);
    return mix(color, tint, fog_amount(world));
}
