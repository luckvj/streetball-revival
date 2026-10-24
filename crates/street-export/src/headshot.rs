//! A head shot rendered from a character's bind pose, for games whose discs
//! keep no portrait the export can use: front view, orthographic, textured,
//! lit from the front, on a transparent background.

use street_assets::character::Character;

/// `size`×`size` RGBA of the head: the square from a little under the neck
/// to the crown, centred on the head. `neck` and `head` are the bones'
/// positions in model space; the character faces +z.
pub fn render(c: &Character, neck: [f32; 3], head: [f32; 3], size: u32) -> Option<Vec<u8>> {
    let (_, hi) = c.bounds()?;
    let top = hi[1];
    let bottom = neck[1] - (top - neck[1]) * 0.25;
    let span = (top - bottom).max(1e-3) * 1.08;
    let centre = [head[0], (top + bottom) / 2.0];
    let s = size as usize;
    let mut rgba = vec![0u8; s * s * 4];
    let mut depth = vec![f32::NEG_INFINITY; s * s];
    let project = |p: [f32; 3]| -> [f32; 3] {
        [
            ((p[0] - centre[0]) / span + 0.5) * size as f32,
            (0.5 - (p[1] - centre[1]) / span) * size as f32,
            p[2],
        ]
    };
    for m in &c.meshes {
        let tex = m.texture.and_then(|t| c.textures.get(t));
        for t in m.indices.as_chunks::<3>().0 {
            let v = t.map(|i| i as usize);
            if v.iter().any(|&i| i >= m.positions.len()) {
                continue;
            }
            let p = v.map(|i| project(m.positions[i]));
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
            if area.abs() < 1e-9 {
                continue;
            }
            let lo_x = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).max(0.0) as usize;
            let hi_x =
                (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).max(0.0) as usize).min(s - 1);
            let lo_y = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).max(0.0) as usize;
            let hi_y =
                (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).max(0.0) as usize).min(s - 1);
            for y in lo_y..=hi_y {
                for x in lo_x..=hi_x {
                    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 =
                        ((p[1][0] - fx) * (p[2][1] - fy) - (p[2][0] - fx) * (p[1][1] - fy)) / area;
                    let w1 =
                        ((p[2][0] - fx) * (p[0][1] - fy) - (p[0][0] - fx) * (p[2][1] - fy)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
                    let at = y * s + x;
                    if z <= depth[at] {
                        continue;
                    }
                    let mut colour = [190.0f32, 190.0, 190.0];
                    if let Some(tex) = tex
                        && m.uvs.len() == m.positions.len()
                        && tex.width > 0
                        && tex.height > 0
                    {
                        let uv = [0, 1].map(|k| {
                            w0 * m.uvs[v[0]][k] + w1 * m.uvs[v[1]][k] + w2 * m.uvs[v[2]][k]
                        });
                        let tx = (uv[0].rem_euclid(1.0) * tex.width as f32) as usize
                            % tex.width as usize;
                        let ty = (uv[1].rem_euclid(1.0) * tex.height as f32) as usize
                            % tex.height as usize;
                        let o = (ty * tex.width as usize + tx) * 4;
                        if tex.rgba[o + 3] < 128 {
                            continue;
                        }
                        colour = [0, 1, 2].map(|k| f32::from(tex.rgba[o + k]));
                    }
                    let n = m.normals.get(v[0]).copied().unwrap_or([0.0, 0.0, 1.0]);
                    let light = 0.6 + 0.4 * n[2].abs().clamp(0.0, 1.0);
                    depth[at] = z;
                    for k in 0..3 {
                        rgba[at * 4 + k] = (colour[k] * light).min(255.0) as u8;
                    }
                    rgba[at * 4 + 3] = 255;
                }
            }
        }
    }
    rgba.iter()
        .skip(3)
        .step_by(4)
        .any(|&a| a > 0)
        .then_some(rgba)
}

#[cfg(test)]
mod tests {
    use street_assets::character::{Character, Mesh};
    use street_assets::eagl::Skeleton;

    #[test]
    fn draws_a_triangle_in_the_frame() {
        let c = Character {
            id: "t:a".into(),
            name: "A".into(),
            height_in: None,
            skeleton: Skeleton::default(),
            meshes: vec![Mesh {
                name: "m".into(),
                positions: vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
                ..Mesh::default()
            }],
            textures: Vec::new(),
            notes: Vec::new(),
        };
        let px = super::render(&c, [0.0, 0.5, 0.0], [0.0, 1.0, 0.0], 16).unwrap();
        // The centre column is drawn, the top corners are not.
        assert_eq!(px[(8 * 16 + 8) * 4 + 3], 255);
        assert_eq!(px[3], 0);
    }
}
