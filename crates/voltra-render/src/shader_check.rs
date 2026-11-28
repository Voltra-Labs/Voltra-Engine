//! Whether a game's shader runs on WebGL2: what the material checks share.

use wgpu::naga;

/// Every entry point of `module` written as WebGL2's GLSL (ES 300), as
/// wgpu's GL backend writes it: what naga cannot write there, WebGL2
/// cannot run.
pub(crate) fn webgl2(module: &naga::Module, info: &naga::valid::ModuleInfo) -> Result<(), String> {
    use ::naga::back::glsl;
    let mut binding_map = glsl::BindingMap::default();
    let mut slot = 0u8;
    for (_, global) in module.global_variables.iter() {
        if let Some(binding) = &global.binding {
            binding_map.insert(*binding, slot);
            slot = slot.saturating_add(1);
        }
    }
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        writer_flags: glsl::WriterFlags::ADJUST_COORDINATE_SPACE
            | glsl::WriterFlags::FORCE_POINT_SIZE,
        binding_map,
        zero_initialize_workgroup_memory: false,
    };
    for entry in &module.entry_points {
        let pipeline = glsl::PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
            multiview: None,
        };
        let mut out = String::new();
        glsl::Writer::new(
            &mut out,
            module,
            info,
            &options,
            &pipeline,
            ::naga::proc::BoundsCheckPolicies::default(),
        )
        .and_then(|mut writer| writer.write().map(|_| ()))
        .map_err(|e| format!("`{}` cannot run on WebGL2 (GLSL ES 300): {e}", entry.name))?;
    }
    Ok(())
}
