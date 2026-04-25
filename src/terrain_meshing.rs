use std::cell::RefCell;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use bevy_voxel_world::{
    custom_meshing::{CHUNK_SIZE_U, VoxelArray},
    prelude::{TextureIndexMapperFn, WorldVoxel},
    rendering::ATTRIBUTE_TEX_INDEX,
};
use block_mesh::{
    OrientedBlockFace, QuadBuffer, RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad, Voxel,
    VoxelVisibility, ilattice::glam::Vec3 as BMVec3,
};
use block_mesh_bgm::{BinaryGreedyQuadsBuffer, binary_greedy_quads, binary_greedy_quads_ao_safe};
use ndshape::{RuntimeShape, Shape};

thread_local! {
    static BINARY_BUFFER: RefCell<BinaryGreedyQuadsBuffer> =
        RefCell::new(BinaryGreedyQuadsBuffer::new());
}

pub fn build_chunk_mesh(
    voxels: VoxelArray<u8>,
    data_shape_in: UVec3,
    mesh_shape_in: UVec3,
    texture_index_mapper: TextureIndexMapperFn<u8>,
    ambient_occlusion: bool,
) -> Mesh {
    let default_shape = UVec3::splat(CHUNK_SIZE_U + 2);
    let data_shape_in = if data_shape_in == UVec3::ZERO {
        default_shape
    } else {
        data_shape_in
    };
    let mesh_shape_in = if mesh_shape_in == UVec3::ZERO {
        default_shape
    } else {
        mesh_shape_in
    };

    let data_shape = RuntimeShape::<u32, 3>::new(data_shape_in.to_array());
    let mesh_shape = RuntimeShape::<u32, 3>::new(mesh_shape_in.to_array());
    let faces = RIGHT_HANDED_Y_UP_CONFIG.faces;

    let voxels_for_mesh: VoxelArray<u8> = if data_shape_in != mesh_shape_in {
        resample_voxels_nearest(voxels.as_ref(), &data_shape, &mesh_shape).into()
    } else {
        voxels
    };

    let max = [
        mesh_shape_in.x.saturating_sub(1),
        mesh_shape_in.y.saturating_sub(1),
        mesh_shape_in.z.saturating_sub(1),
    ];

    BINARY_BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();

        if ambient_occlusion {
            binary_greedy_quads_ao_safe(
                voxels_for_mesh.as_ref(),
                &mesh_shape,
                [0; 3],
                max,
                &faces,
                &mut buffer,
            );
        } else {
            binary_greedy_quads(
                voxels_for_mesh.as_ref(),
                &mesh_shape,
                [0; 3],
                max,
                &faces,
                &mut buffer,
            );
        }

        build_render_mesh(
            &buffer.quads,
            &faces,
            voxels_for_mesh.as_ref(),
            &mesh_shape,
            &texture_index_mapper,
            ambient_occlusion,
        )
    })
}

fn build_render_mesh(
    quads: &QuadBuffer,
    faces: &[OrientedBlockFace; 6],
    voxels: &[WorldVoxel<u8>],
    shape: &RuntimeShape<u32, 3>,
    texture_index_mapper: &TextureIndexMapperFn<u8>,
    ambient_occlusion: bool,
) -> Mesh {
    let mut mesh = RenderMeshBuffers::with_quad_capacity(quads.num_quads());
    let voxel_size = voxel_size_from_shape(shape);
    let mut material_cache = [None; 256];

    for (group, face) in quads.groups.iter().zip(faces.iter().copied()) {
        for quad in group {
            let ao = if ambient_occlusion {
                Some(face_aos(face, quad.minimum, voxels, shape))
            } else {
                None
            };
            mesh.append_quad(
                face,
                quad,
                voxels,
                shape,
                voxel_size,
                texture_index_mapper,
                &mut material_cache,
                ao,
            );
        }
    }

    mesh.build()
}

#[derive(Default)]
struct RenderMeshBuffers {
    indices: Vec<u32>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    tex_coords: Vec<[f32; 2]>,
    material_types: Vec<[u32; 3]>,
    colors: Vec<[f32; 4]>,
}

impl RenderMeshBuffers {
    fn with_quad_capacity(num_quads: usize) -> Self {
        Self {
            indices: Vec::with_capacity(num_quads * 6),
            positions: Vec::with_capacity(num_quads * 4),
            normals: Vec::with_capacity(num_quads * 4),
            tex_coords: Vec::with_capacity(num_quads * 4),
            material_types: Vec::with_capacity(num_quads * 4),
            colors: Vec::with_capacity(num_quads * 4),
        }
    }

    fn append_quad(
        &mut self,
        face: OrientedBlockFace,
        quad: &UnorientedQuad,
        voxels: &[WorldVoxel<u8>],
        shape: &RuntimeShape<u32, 3>,
        voxel_size: BMVec3,
        texture_index_mapper: &TextureIndexMapperFn<u8>,
        material_cache: &mut [Option<[u32; 3]>; 256],
        ao: Option<[u32; 4]>,
    ) {
        self.indices
            .extend_from_slice(&face.quad_mesh_indices(self.positions.len() as u32));

        let corners = face.quad_corners(quad);
        self.positions.extend_from_slice(&corners.map(|corner| {
            let corner = corner.as_vec3();
            let adjusted = voxel_size * (corner - BMVec3::splat(1.0)) + BMVec3::splat(1.0);
            adjusted.to_array()
        }));

        self.normals.extend_from_slice(&face.quad_mesh_normals());

        let u_direction = corners[1].as_vec3() - corners[0].as_vec3();
        let v_direction = corners[2].as_vec3() - corners[0].as_vec3();
        let u_scale = voxel_size.dot(u_direction) / quad.width.max(1) as f32;
        let v_scale = voxel_size.dot(v_direction) / quad.height.max(1) as f32;
        let tex_coords = face
            .tex_coords(RIGHT_HANDED_Y_UP_CONFIG.u_flip_face, true, quad)
            .map(|[u, v]| [u * u_scale, v * v_scale]);
        self.tex_coords.extend_from_slice(&tex_coords);

        let voxel_index = shape.linearize(quad.minimum) as usize;
        let material = match voxels[voxel_index] {
            WorldVoxel::Solid(material) => {
                let cached = &mut material_cache[material as usize];
                if let Some(mapped) = *cached {
                    mapped
                } else {
                    let mapped = texture_index_mapper(material);
                    *cached = Some(mapped);
                    mapped
                }
            }
            _ => [0, 0, 0],
        };
        self.material_types.extend(std::iter::repeat_n(material, 4));

        if let Some(ao) = ao {
            self.colors.extend(ao.map(ao_vertex_color));
        } else {
            self.colors.extend(std::iter::repeat_n([1.0; 4], 4));
        }
    }

    fn build(self) -> Mesh {
        let mut render_mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        render_mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            VertexAttributeValues::Float32x3(self.positions),
        );
        render_mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            VertexAttributeValues::Float32x3(self.normals),
        );
        render_mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            VertexAttributeValues::Float32x2(self.tex_coords),
        );
        render_mesh.insert_attribute(
            ATTRIBUTE_TEX_INDEX,
            VertexAttributeValues::Uint32x3(self.material_types),
        );
        render_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        render_mesh.insert_indices(Indices::U32(self.indices));
        render_mesh
    }
}

fn map_nearest_1d(mesh_i: u32, mesh_dim: u32, data_dim: u32) -> u32 {
    let mesh_inner = (mesh_dim.saturating_sub(2)).max(1);
    let data_inner = (data_dim.saturating_sub(2)).max(1);

    if mesh_inner == data_inner {
        return mesh_i;
    }

    if mesh_i == 0 {
        return 0;
    }
    if mesh_i >= mesh_dim - 1 {
        return data_dim - 1;
    }

    let mesh_steps = (mesh_inner - 1).max(1) as f32;
    let data_steps = (data_inner - 1).max(1) as f32;
    let ratio = data_steps / mesh_steps;
    let inner_idx = (mesh_i - 1) as f32;
    let mapped = (inner_idx * ratio).round();

    (mapped as u32 + 1).min(data_dim - 1)
}

fn resample_voxels_nearest<I: PartialEq + Copy>(
    data_voxels: &[WorldVoxel<I>],
    data_shape: &RuntimeShape<u32, 3>,
    mesh_shape: &RuntimeShape<u32, 3>,
) -> Vec<WorldVoxel<I>>
where
    WorldVoxel<I>: Clone,
{
    let [mx, my, mz] = mesh_shape.as_array();
    let [dx, dy, dz] = data_shape.as_array();
    let x_map: Vec<u32> = (0..mx).map(|x| map_nearest_1d(x, mx, dx)).collect();
    let y_map: Vec<u32> = (0..my).map(|y| map_nearest_1d(y, my, dy)).collect();
    let z_map: Vec<u32> = (0..mz).map(|z| map_nearest_1d(z, mz, dz)).collect();
    let mut out = Vec::with_capacity(mesh_shape.size() as usize);

    for &sz in &z_map {
        for &sy in &y_map {
            for &sx in &x_map {
                let src_lin = data_shape.linearize([sx, sy, sz]);
                out.push(data_voxels[src_lin as usize]);
            }
        }
    }

    out
}

fn voxel_size_from_shape(shape: &RuntimeShape<u32, 3>) -> BMVec3 {
    let [ex, ey, ez] = shape.as_array();
    let ix = (ex.saturating_sub(2)).max(1);
    let iy = (ey.saturating_sub(2)).max(1);
    let iz = (ez.saturating_sub(2)).max(1);

    BMVec3::new(
        CHUNK_SIZE_U as f32 / ix as f32,
        CHUNK_SIZE_U as f32 / iy as f32,
        CHUNK_SIZE_U as f32 / iz as f32,
    )
}

fn ao_vertex_color(ao_value: u32) -> [f32; 4] {
    match ao_value {
        0 => [0.10, 0.10, 0.10, 1.0],
        1 => [0.30, 0.30, 0.30, 1.0],
        2 => [0.50, 0.50, 0.50, 1.0],
        _ => [1.0, 1.0, 1.0, 1.0],
    }
}

fn ao_value(side1: bool, corner: bool, side2: bool) -> u32 {
    match (side1, corner, side2) {
        (true, _, true) => 0,
        (true, true, false) | (false, true, true) => 1,
        (false, false, false) => 3,
        _ => 2,
    }
}

fn side_aos<I: PartialEq>(neighbours: [WorldVoxel<I>; 8]) -> [u32; 4] {
    let opaque = neighbours.map(|voxel| voxel.get_visibility() == VoxelVisibility::Opaque);

    [
        ao_value(opaque[0], opaque[1], opaque[2]),
        ao_value(opaque[2], opaque[3], opaque[4]),
        ao_value(opaque[6], opaque[7], opaque[0]),
        ao_value(opaque[4], opaque[5], opaque[6]),
    ]
}

fn face_aos<I: PartialEq + Copy>(
    face: OrientedBlockFace,
    minimum: [u32; 3],
    voxels: &[WorldVoxel<I>],
    shape: &RuntimeShape<u32, 3>,
) -> [u32; 4] {
    let normal = face.signed_normal();
    let [x, y, z] = minimum;

    match [normal.x, normal.y, normal.z] {
        [-1, 0, 0] => side_aos([
            voxels[shape.linearize([x - 1, y, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x - 1, y, z + 1]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z + 1]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z - 1]) as usize],
        ]),
        [1, 0, 0] => side_aos([
            voxels[shape.linearize([x + 1, y, z - 1]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z - 1]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z - 1]) as usize],
        ]),
        [0, -1, 0] => side_aos([
            voxels[shape.linearize([x, y - 1, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z - 1]) as usize],
        ]),
        [0, 1, 0] => side_aos([
            voxels[shape.linearize([x, y + 1, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z + 1]) as usize],
            voxels[shape.linearize([x, y + 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z - 1]) as usize],
        ]),
        [0, 0, -1] => side_aos([
            voxels[shape.linearize([x - 1, y, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z - 1]) as usize],
            voxels[shape.linearize([x, y - 1, z - 1]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z - 1]) as usize],
            voxels[shape.linearize([x + 1, y, z - 1]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z - 1]) as usize],
            voxels[shape.linearize([x, y + 1, z - 1]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z - 1]) as usize],
        ]),
        [0, 0, 1] => side_aos([
            voxels[shape.linearize([x - 1, y, z + 1]) as usize],
            voxels[shape.linearize([x - 1, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y - 1, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y, z + 1]) as usize],
            voxels[shape.linearize([x + 1, y + 1, z + 1]) as usize],
            voxels[shape.linearize([x, y + 1, z + 1]) as usize],
            voxels[shape.linearize([x - 1, y + 1, z + 1]) as usize],
        ]),
        _ => unreachable!(),
    }
}
