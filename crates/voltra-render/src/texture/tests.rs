use super::*;

#[test]
fn filter_maps_onto_wgpu() {
    assert_eq!(
        wgpu::FilterMode::from(Filter::Linear),
        wgpu::FilterMode::Linear
    );
    assert_eq!(
        wgpu::FilterMode::from(Filter::Nearest),
        wgpu::FilterMode::Nearest
    );
    assert_eq!(Filter::default(), Filter::Linear);
}

#[test]
fn size_mismatch_message_names_both_sizes() {
    let err = TextureError::SizeMismatch {
        expected: 16,
        actual: 12,
        width: 2,
        height: 2,
    };
    let text = err.to_string();
    assert!(text.contains("12"), "{text}");
    assert!(text.contains("16"), "{text}");
    assert!(text.contains("2x2"), "{text}");
}
