use glam::Mat4;
use glam::Vec3;
use std::io::Read;
use std::io::Write;
use std::path::Path;

const SIGNATURE: &[u8; 16] = b"GoBox Collision\n";
const VERSION: u32 = 2;

pub struct Coll {
    pub signature: [u8; 16],
    pub version: u32,
    pub num_tris: u32,
    pub vbuf: Vec<f32>,
}

impl Default for Coll {
    fn default() -> Self {
        Self {
            signature: *SIGNATURE,
            version: VERSION,
            num_tris: 0,
            vbuf: vec![],
        }
    }
}

impl Coll {
    pub const FILE_EXT: &'static str = "coll";

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(24 + self.vbuf.len() * 4);
        self.serialize(&mut out).unwrap();
        out
    }

    pub fn from_bytes(mut bytes: &[u8]) -> std::io::Result<Self> {
        Self::deserialize(&mut bytes)
    }

    pub fn serialize<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        w.write_all(&self.signature)?;
        w.write_all(&self.version.to_le_bytes())?;
        w.write_all(&self.num_tris.to_le_bytes())?;
        for f in &self.vbuf {
            w.write_all(&f.to_le_bytes())?;
        }
        Ok(())
    }

    pub fn deserialize<R: Read>(r: &mut R) -> std::io::Result<Self> {
        let mut signature = [0u8; 16];
        r.read_exact(&mut signature)?;
        if &signature != SIGNATURE {
            panic!("Signature mismatch. This is probably not a collision file.");
        }

        let mut word = [0u8; 4];
        r.read_exact(&mut word)?;
        let version = u32::from_le_bytes(word);
        if version < VERSION {
            panic!("File too old. Use older tool. (File={version}, Tool={VERSION})")
        } else if version > VERSION {
            panic!("File too new. Update your tool. (File={version}, Tool={VERSION})")
        }

        r.read_exact(&mut word)?;
        let num_tris = u32::from_le_bytes(word);

        let count = (num_tris as usize)
            .checked_mul(9)
            .expect("Triangle count isn't a multiple of 9");

        let mut vbuf = Vec::new();
        for _ in 0..count {
            r.read_exact(&mut word)?;
            vbuf.push(f32::from_le_bytes(word));
        }

        Ok(Self {
            signature,
            version,
            num_tris,
            vbuf,
        })
    }

    pub fn from_gltf<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        let gltf::Gltf { document, blob } = gltf::Gltf::open(path).unwrap();

        let buffers = gltf::import_buffers(&document, path.parent(), blob).unwrap();

        let scene = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .expect("gltf contains no scenes");

        let mut vbuf = Vec::new();
        for node in scene.nodes() {
            collect_gltf_geom(node, &Mat4::IDENTITY, &buffers, &mut vbuf);
        }

        let num_tris = u32::try_from(vbuf.len() / 9).expect("triangles overflow");

        Self {
            signature: *SIGNATURE,
            version: VERSION,
            num_tris,
            vbuf,
        }
    }
}

fn collect_gltf_geom(
    node: gltf::Node,
    parent_xform: &Mat4,
    buffers: &[gltf::buffer::Data],
    vbuf: &mut Vec<f32>,
) {
    let xform = parent_xform * Mat4::from_cols_array_2d(&node.transform().matrix());

    let is_coll = node
        .name()
        .is_some_and(|n| n.ends_with("-colonly") || n.ends_with("-col"));

    if is_coll && let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let mode = prim.mode();
            if !matches!(
                mode,
                gltf::mesh::Mode::Triangles | gltf::mesh::Mode::TriangleStrip
            ) {
                continue;
            }

            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
            let positions: Vec<Vec3> = reader
                .read_positions()
                .expect("prim has no pos?")
                .map(Vec3::from_array)
                .collect();

            let indices: Vec<u32> = reader
                .read_indices()
                .map(|i| i.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let mut tris: Vec<[u32; 3]> = Vec::new();
            // NOTICE: apparently godot and gltf use opposite windings.
            match mode {
                gltf::mesh::Mode::Triangles => {
                    tris.extend(indices.chunks_exact(3).map(|c| [c[1], c[0], c[2]]));
                }
                gltf::mesh::Mode::TriangleStrip => {
                    for i in 0..indices.len() - 2 {
                        tris.push(if i % 2 == 0 {
                            [indices[i + 1], indices[i], indices[i + 2]]
                        } else {
                            [indices[i], indices[i + 1], indices[i + 2]]
                        });
                    }
                }
                _ => unreachable!(),
            }

            for tri in tris {
                for idx in tri {
                    let p = positions.get(idx as usize).unwrap();
                    vbuf.extend_from_slice(&xform.transform_point3(*p).to_array());
                }
            }
        }
    }

    for child in node.children() {
        collect_gltf_geom(child, &xform, buffers, vbuf);
    }
}
