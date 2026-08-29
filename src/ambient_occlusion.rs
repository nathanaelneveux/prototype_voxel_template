use block_mesh::{OrientedBlockFace, UnorientedQuad};

/// Precomputed linear offsets for sampling the AO neighborhood of one face.
///
/// Construct one sampler per face group, outside the quad loop. `strides` must
/// describe a dense linear voxel array in X/Y/Z order.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceAoSampler {
    strides: [usize; 3],
    neighbor_offsets: [isize; 8],
    backward_extent: usize,
    forward_extent: usize,
}

impl FaceAoSampler {
    pub(crate) fn new(face: OrientedBlockFace, strides: [usize; 3]) -> Self {
        let unit_quad = UnorientedQuad {
            minimum: [0; 3],
            width: 1,
            height: 1,
        };
        let corners = face.quad_corners(&unit_quad);
        let normal = face.signed_normal();
        let u = corners[1].as_ivec3() - corners[0].as_ivec3();
        let v = corners[2].as_ivec3() - corners[0].as_ivec3();

        // Ring order around the exterior sample plane:
        //
        //     7 -- 6 -- 5
        //     |         |
        //     0         4
        //     |         |
        //     1 -- 2 -- 3
        //
        // This matches block_mesh's minU/minV, maxU/minV, minU/maxV,
        // maxU/maxV vertex order.
        let neighbor_offsets = [
            normal - u,
            normal - u - v,
            normal - v,
            normal + u - v,
            normal + u,
            normal + u + v,
            normal + v,
            normal - u + v,
        ]
        .map(|direction| vector_offset(direction.to_array(), strides));
        let min_offset = *neighbor_offsets.iter().min().unwrap();
        let max_offset = *neighbor_offsets.iter().max().unwrap();

        Self {
            strides,
            neighbor_offsets,
            backward_extent: min_offset.min(0).unsigned_abs(),
            forward_extent: max_offset.max(0) as usize,
        }
    }

    /// Samples all four AO values after checking the padded-neighborhood
    /// contract once. The eight reads can then omit repeated bounds checks.
    #[inline]
    pub(crate) fn sample<T>(
        &self,
        voxels: &[T],
        minimum: [u32; 3],
        is_opaque: impl Fn(&T) -> bool,
    ) -> [u8; 4] {
        let base_index = minimum[0] as usize * self.strides[0]
            + minimum[1] as usize * self.strides[1]
            + minimum[2] as usize * self.strides[2];
        assert!(
            base_index >= self.backward_extent
                && self.forward_extent < voxels.len()
                && base_index < voxels.len() - self.forward_extent,
            "AO neighborhood is outside the padded voxel array"
        );

        let mut opaque_mask = 0u8;
        for (bit, offset) in self.neighbor_offsets.into_iter().enumerate() {
            let index = base_index.wrapping_add_signed(offset);
            // SAFETY: the backward/forward range check above covers every
            // precomputed neighbor offset.
            opaque_mask |= u8::from(is_opaque(unsafe { voxels.get_unchecked(index) })) << bit;
        }

        AO_BY_RING[opaque_mask as usize]
    }
}

#[inline]
fn vector_offset([x, y, z]: [i32; 3], strides: [usize; 3]) -> isize {
    [x, y, z]
        .into_iter()
        .zip(strides)
        .try_fold(0isize, |offset, (component, stride)| {
            let stride = isize::try_from(stride).ok()?;
            offset.checked_add((component as isize).checked_mul(stride)?)
        })
        .expect("AO strides exceed the supported range")
}

const AO_BY_RING: [[u8; 4]; 256] = build_ao_lookup();

const fn build_ao_lookup() -> [[u8; 4]; 256] {
    let mut lookup = [[0; 4]; 256];
    let mut mask = 0;
    while mask < 256 {
        lookup[mask] = [
            masked_vertex_ao(mask, 0, 1, 2),
            masked_vertex_ao(mask, 2, 3, 4),
            masked_vertex_ao(mask, 6, 7, 0),
            masked_vertex_ao(mask, 4, 5, 6),
        ];
        mask += 1;
    }
    lookup
}

const fn masked_vertex_ao(mask: usize, side1: usize, corner: usize, side2: usize) -> u8 {
    vertex_ao(
        mask & (1 << side1) != 0,
        mask & (1 << corner) != 0,
        mask & (1 << side2) != 0,
    )
}

const fn vertex_ao(side1: bool, corner: bool, side2: bool) -> u8 {
    if side1 && side2 {
        0
    } else {
        3 - side1 as u8 - corner as u8 - side2 as u8
    }
}

#[cfg(test)]
mod tests {
    use block_mesh::{OrientedBlockFace, RIGHT_HANDED_Y_UP_CONFIG, SignedAxis, UnorientedQuad};

    use super::{FaceAoSampler, vertex_ao};

    const EDGE: usize = 7;
    const STRIDES: [usize; 3] = [1, EDGE, EDGE * EDGE];

    #[test]
    fn sampler_matches_coordinate_reference_for_every_face_orientation() {
        let voxels = core::array::from_fn::<_, { EDGE * EDGE * EDGE }, _>(|index| {
            let x = index % EDGE;
            let y = index / EDGE % EDGE;
            let z = index / (EDGE * EDGE);
            (x * 17 + y * 7 + z * 13) % 5 <= 1
        });
        let minimum = [3, 3, 3];
        let mut faces = RIGHT_HANDED_Y_UP_CONFIG.faces.to_vec();
        faces.extend([
            OrientedBlockFace::canonical(SignedAxis::PosX),
            OrientedBlockFace::canonical(SignedAxis::PosY),
            OrientedBlockFace::canonical(SignedAxis::PosZ),
        ]);

        for face in faces {
            let sampler = FaceAoSampler::new(face, STRIDES);
            let actual = sampler.sample(&voxels, minimum, |&opaque| opaque);
            let expected = coordinate_reference(face, minimum, &voxels);
            assert_eq!(actual, expected);
        }
    }

    fn coordinate_reference(
        face: OrientedBlockFace,
        minimum: [u32; 3],
        voxels: &[bool; EDGE * EDGE * EDGE],
    ) -> [u8; 4] {
        let quad = UnorientedQuad {
            minimum,
            width: 1,
            height: 1,
        };
        let corners = face.quad_corners(&quad);
        let normal = face.signed_normal();
        let u = corners[1].as_ivec3() - corners[0].as_ivec3();
        let v = corners[2].as_ivec3() - corners[0].as_ivec3();
        let center =
            block_mesh::ilattice::glam::IVec3::from(minimum.map(|value| value as i32)) + normal;
        let coords = [
            center - u,
            center - u - v,
            center - v,
            center + u - v,
            center + u,
            center + u + v,
            center + v,
            center - u + v,
        ];
        let opaque = coords.map(|coord| {
            let [x, y, z] = coord.to_array().map(|value| value as usize);
            voxels[x + y * EDGE + z * EDGE * EDGE]
        });

        [
            vertex_ao(opaque[0], opaque[1], opaque[2]),
            vertex_ao(opaque[2], opaque[3], opaque[4]),
            vertex_ao(opaque[6], opaque[7], opaque[0]),
            vertex_ao(opaque[4], opaque[5], opaque[6]),
        ]
    }
}
