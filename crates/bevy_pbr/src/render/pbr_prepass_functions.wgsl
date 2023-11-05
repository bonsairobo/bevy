#define_import_path bevy_pbr::pbr_prepass_functions

#import bevy_pbr::{
    prepass_io::VertexOutput,
    prepass_bindings::previous_view_proj,
    mesh_view_bindings::view,
    pbr_bindings,
    pbr_types,
}

// Cutoff used for the premultiplied alpha modes BLEND and ADD.
const PREMULTIPLIED_ALPHA_CUTOFF = 0.05;

// We can use a simplified version of alpha_discard() here since we only need to handle the alpha_cutoff
fn prepass_alpha_discard(material_flags: u32, alpha_cutoff: f32, output_color: vec4<f32>) {
    let alpha_mode = material_flags & bevy_pbr::pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if alpha_mode == bevy_pbr::pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK {
        if output_color.a < alpha_cutoff {
            discard;
        }
    } else if (alpha_mode == bevy_pbr::pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_BLEND || alpha_mode == bevy_pbr::pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ADD) {
        if output_color.a < PREMULTIPLIED_ALPHA_CUTOFF {
            discard;
        }
    } else if alpha_mode == bevy_pbr::pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_PREMULTIPLIED {
        if all(output_color < vec4(PREMULTIPLIED_ALPHA_CUTOFF)) {
            discard;
        }
    }
}

fn prepass_sample_color_and_alpha_discard(in: FragmentInput) {

#ifdef MAY_DISCARD
    var output_color: vec4<f32> = bevy_pbr::pbr_bindings::material.base_color;

#ifdef VERTEX_UVS
    if (bevy_pbr::pbr_bindings::material.flags & bevy_pbr::pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        output_color = output_color * textureSampleBias(bevy_pbr::pbr_bindings::base_color_texture, bevy_pbr::pbr_bindings::base_color_sampler, in.uv, bevy_pbr::prepass_bindings::view.mip_bias);
    }
#endif // VERTEX_UVS

    prepass_alpha_discard(
        bevy_pbr::pbr_bindings::material.flags,
        bevy_pbr::pbr_bindings::material.alpha_cutoff,
        output_color
    );

#endif // MAY_DISCARD
}

#ifdef MOTION_VECTOR_PREPASS
fn calculate_motion_vector(world_position: vec4<f32>, previous_world_position: vec4<f32>) -> vec2<f32> {
    let clip_position_t = view.unjittered_view_proj * world_position;
    let clip_position = clip_position_t.xy / clip_position_t.w;
    let previous_clip_position_t = previous_view_proj * previous_world_position;
    let previous_clip_position = previous_clip_position_t.xy / previous_clip_position_t.w;
    // These motion vectors are used as offsets to UV positions and are stored
    // in the range -1,1 to allow offsetting from the one corner to the
    // diagonally-opposite corner in UV coordinates, in either direction.
    // A difference between diagonally-opposite corners of clip space is in the
    // range -2,2, so this needs to be scaled by 0.5. And the V direction goes
    // down where clip space y goes up, so y needs to be flipped.
    return (clip_position - previous_clip_position) * vec2(0.5, -0.5);
}
#endif // MOTION_VECTOR_PREPASS
