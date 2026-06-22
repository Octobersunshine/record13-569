use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::path::PathBuf;

use crate::error::AppError;
use crate::model::{CompressionOptions, MeshInfo};

#[derive(Debug, Clone)]
pub struct CompressionResult {
    pub output_path: PathBuf,
    pub original_info: MeshInfo,
    pub compressed_info: MeshInfo,
}

#[derive(Debug, Clone)]
pub struct Compressor;

impl Compressor {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_mesh(&self, path: &PathBuf) -> Result<MeshInfo, AppError> {
        let file_size = std::fs::metadata(path).map_err(AppError::Io)?.len();
        let mesh = Self::load_mesh(path)?;
        Ok(MeshInfo {
            vertex_count: mesh.vertices.len(),
            face_count: mesh.faces.len(),
            file_size_bytes: file_size,
        })
    }

    fn get_extension(path: &PathBuf) -> String {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default()
    }

    fn load_mesh(path: &PathBuf) -> Result<RawMesh, AppError> {
        let ext = Self::get_extension(path);
        match ext.as_str() {
            "obj" => Self::load_obj(path),
            "stl" => Self::load_stl(path),
            other => Err(AppError::UnsupportedFormat(format!(
                "Format '{}' is not yet supported for simplification. Supported: obj, stl",
                other
            ))),
        }
    }

    fn load_obj(path: &PathBuf) -> Result<RawMesh, AppError> {
        let (models, materials) = tobj::load_obj(
            path,
            &tobj::LoadOptions {
                single_index: true,
                triangulate: true,
                ignore_points: true,
                ignore_lines: true,
            },
        )
        .map_err(|e| AppError::ModelLoad(format!("Failed to load OBJ: {}", e)))?;

        let _ = materials;

        let mut vertices: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut faces: Vec<[u32; 3]> = Vec::new();

        for model in models.iter() {
            let mesh = &model.mesh;
            let v_start = vertices.len() as u32;

            for i in 0..mesh.positions.len() / 3 {
                vertices.push([
                    mesh.positions[i * 3],
                    mesh.positions[i * 3 + 1],
                    mesh.positions[i * 3 + 2],
                ]);
            }

            if !mesh.normals.is_empty() {
                for i in 0..mesh.normals.len() / 3 {
                    normals.push([
                        mesh.normals[i * 3],
                        mesh.normals[i * 3 + 1],
                        mesh.normals[i * 3 + 2],
                    ]);
                }
            }

            if !mesh.texcoords.is_empty() {
                for i in 0..mesh.texcoords.len() / 2 {
                    uvs.push([mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]]);
                }
            }

            for i in (0..mesh.indices.len()).step_by(3) {
                if i + 2 < mesh.indices.len() {
                    faces.push([
                        v_start + mesh.indices[i],
                        v_start + mesh.indices[i + 1],
                        v_start + mesh.indices[i + 2],
                    ]);
                }
            }
        }

        while normals.len() < vertices.len() {
            normals.push([0.0, 0.0, 0.0]);
        }
        while uvs.len() < vertices.len() {
            uvs.push([0.0, 0.0]);
        }

        if vertices.is_empty() || faces.is_empty() {
            return Err(AppError::ModelLoad("OBJ has no vertices or faces".into()));
        }

        Ok(RawMesh {
            vertices,
            normals,
            uvs,
            faces,
        })
    }

    fn load_stl(path: &PathBuf) -> Result<RawMesh, AppError> {
        let file = std::fs::File::open(path).map_err(AppError::Io)?;
        let mut buf_reader = std::io::BufReader::new(file);
        let stl = stl_io::read_stl(&mut buf_reader)
            .map_err(|e| AppError::ModelLoad(format!("Failed to load STL: {}", e)))?;

        let mut vertices: Vec<[f32; 3]> = Vec::with_capacity(stl.vertices.len());
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(stl.faces.len() * 3);
        let mut face_vertex_indices: Vec<[usize; 3]> = Vec::with_capacity(stl.faces.len());

        for v in stl.vertices.iter() {
            vertices.push([v[0], v[1], v[2]]);
        }

        for face in stl.faces.iter() {
            face_vertex_indices.push(face.vertices);
            let n = face.normal;
            for _ in 0..3 {
                normals.push([n[0], n[1], n[2]]);
            }
        }

        let mut vertex_map: HashMap<usize, usize> = HashMap::new();
        let mut dedup_vertices: Vec<[f32; 3]> = Vec::new();
        let mut dedup_normals: Vec<[f32; 3]> = Vec::new();
        let mut faces: Vec<[u32; 3]> = Vec::with_capacity(face_vertex_indices.len());

        for (fi, orig_face) in face_vertex_indices.iter().enumerate() {
            let mut new_face = [0u32; 3];
            for (local_j, &orig_vidx) in orig_face.iter().enumerate() {
                let global_nidx = fi * 3 + local_j;
                let key = orig_vidx * 1_000_000 + global_nidx;

                if let Some(&new_idx) = vertex_map.get(&key) {
                    new_face[local_j] = new_idx as u32;
                } else {
                    let new_idx = dedup_vertices.len();
                    dedup_vertices.push(vertices[orig_vidx]);
                    dedup_normals.push(normals[global_nidx]);
                    vertex_map.insert(key, new_idx);
                    new_face[local_j] = new_idx as u32;
                }
            }
            faces.push(new_face);
        }

        let uvs = vec![[0.0f32, 0.0]; dedup_vertices.len()];

        if dedup_vertices.is_empty() || faces.is_empty() {
            return Err(AppError::ModelLoad("STL has no vertices or faces".into()));
        }

        Ok(RawMesh {
            vertices: dedup_vertices,
            normals: dedup_normals,
            uvs,
            faces,
        })
    }

    pub fn compress(
        &self,
        input_path: &PathBuf,
        output_path: &PathBuf,
        options: &CompressionOptions,
        progress_callback: impl Fn(f32),
    ) -> Result<CompressionResult, AppError> {
        progress_callback(0.05);

        let input_size = std::fs::metadata(input_path)
            .map_err(AppError::Io)?
            .len();

        progress_callback(0.1);

        let raw_mesh = Self::load_mesh(input_path)?;

        let orig_vcount = raw_mesh.vertices.len();
        let orig_fcount = raw_mesh.faces.len();

        progress_callback(0.2);

        let quality = options.quality.clamp(0.01, 1.0);
        let target_faces = if let Some(tf) = options.target_face_count {
            tf.min(orig_fcount).max(4)
        } else if let Some(tv) = options.target_vertex_count {
            ((tv as f64 * 2.0) as usize).min(orig_fcount).max(4)
        } else {
            ((orig_fcount as f64) * (quality as f64)).max(4.0) as usize
        };

        progress_callback(0.25);

        let simplified = if target_faces >= orig_fcount {
            raw_mesh.clone()
        } else {
            Self::simplify_mesh_qem(
                &raw_mesh,
                target_faces,
                options.preserve_borders,
                options.preserve_uvs,
                &progress_callback,
            )?
        };

        progress_callback(0.9);

        let obj_content = Self::write_obj(&simplified)?;
        std::fs::write(output_path, obj_content).map_err(AppError::Io)?;

        progress_callback(0.95);

        let output_size = std::fs::metadata(output_path)
            .map_err(AppError::Io)?
            .len();

        let compressed_info = MeshInfo {
            vertex_count: simplified.vertices.len(),
            face_count: simplified.faces.len(),
            file_size_bytes: output_size,
        };

        let original_info = MeshInfo {
            vertex_count: orig_vcount,
            face_count: orig_fcount,
            file_size_bytes: input_size,
        };

        progress_callback(1.0);

        Ok(CompressionResult {
            output_path: output_path.clone(),
            original_info,
            compressed_info,
        })
    }

    fn simplify_mesh_qem(
        mesh: &RawMesh,
        target_faces: usize,
        preserve_borders: bool,
        preserve_uvs: bool,
        progress: &impl Fn(f32),
    ) -> Result<RawMesh, AppError> {
        let _ = preserve_uvs;

        let mut verts: Vec<[f64; 3]> = mesh
            .vertices
            .iter()
            .map(|v| [v[0] as f64, v[1] as f64, v[2] as f64])
            .collect();
        let mut faces: Vec<[usize; 3]> = mesh
            .faces
            .iter()
            .map(|f| [f[0] as usize, f[1] as usize, f[2] as usize])
            .collect();

        let num_verts = verts.len();
        let num_faces = faces.len();

        if num_faces <= target_faces || num_verts < 4 {
            return Ok(mesh.clone());
        }

        progress(0.3);

        let vertex_faces = Self::build_vertex_face_map(num_verts, &faces);
        let border_vertices = Self::detect_borders(num_verts, &faces, &vertex_faces);

        progress(0.35);

        let mut quadrics: Vec<Matrix4> = (0..num_verts).map(|_| Matrix4::zero()).collect();

        for face in &faces {
            let q = Self::compute_face_quadric(
                &verts[face[0]],
                &verts[face[1]],
                &verts[face[2]],
            );
            quadrics[face[0]].accumulate(&q);
            quadrics[face[1]].accumulate(&q);
            quadrics[face[2]].accumulate(&q);
        }

        progress(0.4);

        let mut edges: Vec<Edge> = Vec::new();
        let mut edge_set: HashSet<(usize, usize)> = HashSet::new();

        for (_, face) in faces.iter().enumerate() {
            for i in 0..3 {
                let a = face[i].min(face[(i + 1) % 3]);
                let b = face[i].max(face[(i + 1) % 3]);
                let key = (a, b);
                if !edge_set.contains(&key) {
                    edge_set.insert(key);

                    if preserve_borders
                        && (border_vertices.contains(&a) || border_vertices.contains(&b))
                    {
                        let is_boundary_edge = {
                            let shared_faces_a: HashSet<usize> =
                                vertex_faces[&a].iter().cloned().collect();
                            let shared_faces_b: HashSet<usize> =
                                vertex_faces[&b].iter().cloned().collect();
                            shared_faces_a.intersection(&shared_faces_b).count() < 2
                        };
                        if is_boundary_edge {
                            continue;
                        }
                    }

                    let cost =
                        Self::compute_edge_error(&verts[a], &verts[b], &quadrics[a], &quadrics[b]);
                    edges.push(Edge { a, b, cost });
                }
            }
        }

        progress(0.45);

        edges.sort_by(|a, b| a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal));

        let mut alive_vert: Vec<bool> = vec![true; num_verts];
        let mut alive_face: Vec<bool> = vec![true; num_faces];
        let mut remap: Vec<usize> = (0..num_verts).collect();
        let mut current_faces = num_faces;

        let mut collapsed_edges = 0usize;
        let total_edges_to_collapse = (num_faces - target_faces).max(1);

        for edge in edges.iter() {
            if current_faces <= target_faces {
                break;
            }
            let a = edge.a;
            let b = edge.b;
            if !alive_vert[a] || !alive_vert[b] {
                continue;
            }
            if remap[a] != a || remap[b] != b {
                continue;
            }

            let target = Self::compute_optimal_position(
                &verts[a],
                &verts[b],
                &quadrics[a],
                &quadrics[b],
            );

            verts[a] = target;
            let qb = quadrics[b];
            quadrics[a].accumulate(&qb);
            alive_vert[b] = false;
            remap[b] = a;

            let faces_a = vertex_faces.get(&a).cloned().unwrap_or_default();
            let faces_b = vertex_faces.get(&b).cloned().unwrap_or_default();
            let mut affected_faces: HashSet<usize> = HashSet::new();
            for f in faces_a {
                affected_faces.insert(f);
            }
            for f in faces_b {
                affected_faces.insert(f);
            }

            for &fi in affected_faces.iter() {
                if !alive_face[fi] {
                    continue;
                }
                let mut f = faces[fi];
                let mut changed = false;
                for j in 0..3 {
                    if f[j] == b {
                        f[j] = a;
                        changed = true;
                    }
                    if remap[f[j]] != f[j] {
                        f[j] = remap[f[j]];
                        changed = true;
                    }
                }
                let unique = {
                    let mut s = std::collections::HashSet::new();
                    s.insert(f[0]);
                    s.insert(f[1]);
                    s.insert(f[2]);
                    s.len() == 3
                };
                if !unique {
                    alive_face[fi] = false;
                    current_faces -= 1;
                } else if changed {
                    faces[fi] = f;
                }
            }

            collapsed_edges += 1;
            if collapsed_edges % 1000 == 0 {
                let p = 0.45
                    + (collapsed_edges as f32 / total_edges_to_collapse as f32) * 0.40;
                progress(p.min(0.88));
            }
        }

        progress(0.88);

        let mut new_index: Vec<usize> = vec![0; num_verts];
        let mut new_verts: Vec<[f32; 3]> = Vec::new();
        let mut new_normals: Vec<[f32; 3]> = Vec::new();
        let mut new_uvs: Vec<[f32; 2]> = Vec::new();
        let mut count = 0usize;
        for i in 0..num_verts {
            if alive_vert[i] && remap[i] == i {
                new_index[i] = count;
                new_verts
                    .push([verts[i][0] as f32, verts[i][1] as f32, verts[i][2] as f32]);
                if i < mesh.normals.len() {
                    new_normals.push(mesh.normals[i]);
                } else {
                    new_normals.push([0.0, 0.0, 0.0]);
                }
                if i < mesh.uvs.len() {
                    new_uvs.push(mesh.uvs[i]);
                } else {
                    new_uvs.push([0.0, 0.0]);
                }
                count += 1;
            }
        }

        let mut new_faces: Vec<[u32; 3]> = Vec::new();
        for (fi, f) in faces.iter().enumerate() {
            if alive_face[fi] {
                let a = new_index[remap[f[0]]];
                let b = new_index[remap[f[1]]];
                let c = new_index[remap[f[2]]];
                if a != b && b != c && a != c {
                    new_faces.push([a as u32, b as u32, c as u32]);
                }
            }
        }

        progress(0.89);

        Ok(RawMesh {
            vertices: new_verts,
            normals: new_normals,
            uvs: new_uvs,
            faces: new_faces,
        })
    }

    fn build_vertex_face_map(
        num_verts: usize,
        faces: &[[usize; 3]],
    ) -> HashMap<usize, Vec<usize>> {
        let mut map: HashMap<usize, Vec<usize>> = HashMap::with_capacity(num_verts);
        for (fi, face) in faces.iter().enumerate() {
            for j in 0..3 {
                map.entry(face[j]).or_default().push(fi);
            }
        }
        map
    }

    fn detect_borders(
        num_verts: usize,
        faces: &[[usize; 3]],
        vertex_faces: &HashMap<usize, Vec<usize>>,
    ) -> HashSet<usize> {
        let mut borders = HashSet::new();

        for vi in 0..num_verts {
            if let Some(fs) = vertex_faces.get(&vi) {
                let edge_counts: HashMap<(usize, usize), usize> = fs
                    .iter()
                    .flat_map(|&fi| {
                        let f = faces[fi];
                        let mut out = Vec::new();
                        for k in 0..3 {
                            let x = f[k];
                            let y = f[(k + 1) % 3];
                            if x == vi || y == vi {
                                let e = (x.min(y), x.max(y));
                                out.push(e);
                            }
                        }
                        out.into_iter()
                    })
                    .fold(HashMap::new(), |mut acc, e| {
                        *acc.entry(e).or_insert(0) += 1;
                        acc
                    });

                for (_, count) in edge_counts.iter() {
                    if *count == 1 {
                        borders.insert(vi);
                        break;
                    }
                }
            }
        }

        borders
    }

    fn compute_face_quadric(v0: &[f64; 3], v1: &[f64; 3], v2: &[f64; 3]) -> Matrix4 {
        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let n_len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let (a, b, c, d) = if n_len > 1e-12 {
            (
                n[0] / n_len,
                n[1] / n_len,
                n[2] / n_len,
                -(n[0] * v0[0] + n[1] * v0[1] + n[2] * v0[2]) / n_len,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        Matrix4::from_plane(a, b, c, d)
    }

    fn compute_edge_error(
        va: &[f64; 3],
        vb: &[f64; 3],
        qa: &Matrix4,
        qb: &Matrix4,
    ) -> f64 {
        let v = [
            (va[0] + vb[0]) * 0.5,
            (va[1] + vb[1]) * 0.5,
            (va[2] + vb[2]) * 0.5,
        ];
        let q = qa.combined(qb);
        q.evaluate(&v)
    }

    fn compute_optimal_position(
        va: &[f64; 3],
        vb: &[f64; 3],
        qa: &Matrix4,
        qb: &Matrix4,
    ) -> [f64; 3] {
        let q = qa.combined(qb);
        if let Some(v) = q.solve() {
            let e0 = q.evaluate(va);
            let e1 = q.evaluate(vb);
            let ev = q.evaluate(&v);
            if ev <= e0 && ev <= e1 {
                return v;
            }
        }
        let mid = [
            (va[0] + vb[0]) * 0.5,
            (va[1] + vb[1]) * 0.5,
            (va[2] + vb[2]) * 0.5,
        ];
        let em = q.evaluate(&mid);
        let e0 = q.evaluate(va);
        let e1 = q.evaluate(vb);
        if em < e0 && em < e1 {
            mid
        } else if e0 < e1 {
            *va
        } else {
            *vb
        }
    }

    fn write_obj(mesh: &RawMesh) -> Result<String, AppError> {
        let mut out =
            String::with_capacity(mesh.vertices.len() * 30 + mesh.faces.len() * 40);
        writeln!(out, "# Model exported by model-compressor").ok();
        writeln!(
            out,
            "# Vertices: {}, Faces: {}",
            mesh.vertices.len(),
            mesh.faces.len()
        )
        .ok();
        writeln!(out, "o Mesh").ok();

        for v in mesh.vertices.iter() {
            writeln!(out, "v {:.6} {:.6} {:.6}", v[0], v[1], v[2]).ok();
        }

        let has_uv = mesh.uvs.len() >= mesh.vertices.len();
        let has_normals = mesh.normals.len() >= mesh.vertices.len();

        if has_uv {
            for tc in mesh.uvs.iter().take(mesh.vertices.len()) {
                writeln!(out, "vt {:.6} {:.6}", tc[0], 1.0 - tc[1]).ok();
            }
        }

        if has_normals {
            for n in mesh.normals.iter().take(mesh.vertices.len()) {
                writeln!(out, "vn {:.6} {:.6} {:.6}", n[0], n[1], n[2]).ok();
            }
        }

        for f in mesh.faces.iter() {
            let i1 = f[0] + 1;
            let i2 = f[1] + 1;
            let i3 = f[2] + 1;
            match (has_uv, has_normals) {
                (true, true) => writeln!(
                    out, "f {}/{}/{} {}/{}/{} {}/{}/{}",
                    i1, i1, i1, i2, i2, i2, i3, i3, i3
                ),
                (true, false) => writeln!(
                    out, "f {}/{} {}/{} {}/{}",
                    i1, i1, i2, i2, i3, i3
                ),
                (false, true) => writeln!(
                    out, "f {}//{} {}//{} {}//{}",
                    i1, i1, i2, i2, i3, i3
                ),
                (false, false) => writeln!(out, "f {} {} {}", i1, i2, i3),
            }
            .ok();
        }

        Ok(out)
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct RawMesh {
    vertices: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    faces: Vec<[u32; 3]>,
}

#[derive(Debug, Clone)]
struct Edge {
    a: usize,
    b: usize,
    cost: f64,
}

#[derive(Debug, Clone, Copy)]
struct Matrix4 {
    data: [[f64; 4]; 4],
}

impl Matrix4 {
    fn zero() -> Self {
        Self { data: [[0.0; 4]; 4] }
    }

    fn from_plane(a: f64, b: f64, c: f64, d: f64) -> Self {
        let mut m = Self::zero();
        m.data[0][0] = a * a;
        m.data[0][1] = a * b;
        m.data[0][2] = a * c;
        m.data[0][3] = a * d;
        m.data[1][0] = b * a;
        m.data[1][1] = b * b;
        m.data[1][2] = b * c;
        m.data[1][3] = b * d;
        m.data[2][0] = c * a;
        m.data[2][1] = c * b;
        m.data[2][2] = c * c;
        m.data[2][3] = c * d;
        m.data[3][0] = d * a;
        m.data[3][1] = d * b;
        m.data[3][2] = d * c;
        m.data[3][3] = d * d;
        m
    }

    fn accumulate(&mut self, other: &Self) {
        for i in 0..4 {
            for j in 0..4 {
                self.data[i][j] += other.data[i][j];
            }
        }
    }

    fn combined(&self, other: &Self) -> Self {
        let mut m = Self::zero();
        for i in 0..4 {
            for j in 0..4 {
                m.data[i][j] = self.data[i][j] + other.data[i][j];
            }
        }
        m
    }

    fn evaluate(&self, v: &[f64; 3]) -> f64 {
        let x = v[0];
        let y = v[1];
        let z = v[2];
        let w = 1.0;
        let r0 = x * self.data[0][0] + y * self.data[0][1] + z * self.data[0][2] + w * self.data[0][3];
        let r1 = x * self.data[1][0] + y * self.data[1][1] + z * self.data[1][2] + w * self.data[1][3];
        let r2 = x * self.data[2][0] + y * self.data[2][1] + z * self.data[2][2] + w * self.data[2][3];
        let r3 = x * self.data[3][0] + y * self.data[3][1] + z * self.data[3][2] + w * self.data[3][3];
        x * r0 + y * r1 + z * r2 + w * r3
    }

    fn solve(&self) -> Option<[f64; 3]> {
        let mut a = [[0.0; 3]; 3];
        let mut b = [0.0; 3];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] = self.data[i][j];
            }
            b[i] = -self.data[i][3];
        }
        solve_3x3(&a, &b)
    }
}

fn solve_3x3(a: &[[f64; 3]; 3], b: &[f64; 3]) -> Option<[f64; 3]> {
    let mut m = [
        [a[0][0], a[0][1], a[0][2], b[0]],
        [a[1][0], a[1][1], a[1][2], b[1]],
        [a[2][0], a[2][1], a[2][2], b[2]],
    ];

    for col in 0..3 {
        let mut max_row = col;
        let mut max_val = m[col][col].abs();
        for row in (col + 1)..3 {
            let v = m[row][col].abs();
            if v > max_val {
                max_val = v;
                max_row = row;
            }
        }
        if max_val < 1e-12 {
            return None;
        }
        if max_row != col {
            m.swap(col, max_row);
        }
        let pivot = m[col][col];
        for j in col..4 {
            m[col][j] /= pivot;
        }
        for row in 0..3 {
            if row != col {
                let factor = m[row][col];
                if factor.abs() > 1e-18 {
                    for j in col..4 {
                        m[row][j] -= factor * m[col][j];
                    }
                }
            }
        }
    }

    Some([m[0][3], m[1][3], m[2][3]])
}
