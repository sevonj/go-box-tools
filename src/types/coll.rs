use glam::Mat4;
use glam::Vec3;
use std::io::Read;
use std::io::Write;
use std::path::Path;

const SIGNATURE: &[u8; 16] = b"GoBox Collision\n";
const VERSION: u32 = 3;
const TOOL_MINIMUM_VERSION: u32 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollType {
    Convex = 0,
    Concave = 1,
}

#[derive(Debug)]
pub struct Coll {
    pub signature: [u8; 16],
    pub version: u32,
    pub coll_type: CollType,
    pub num_verts: u32,
    pub vbuf: Vec<f32>,
}

impl Coll {
    pub const FILE_EXT: &'static str = "coll";

    pub fn new_convex() -> Self {
        Self {
            signature: *SIGNATURE,
            version: VERSION,
            coll_type: CollType::Convex,
            num_verts: 0,
            vbuf: vec![],
        }
    }

    pub fn new_concave() -> Self {
        Self {
            signature: *SIGNATURE,
            version: VERSION,
            coll_type: CollType::Concave,
            num_verts: 0,
            vbuf: vec![],
        }
    }

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
        w.write_all(&(self.coll_type as i32).to_le_bytes())?;
        w.write_all(&self.num_verts.to_le_bytes())?;
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
            panic!(
                "File too old. Use older tool. (File={version}, Tool Minimum={TOOL_MINIMUM_VERSION})"
            )
        } else if version > VERSION {
            panic!("File too new. Update your tool. (File={version}, Tool={VERSION})")
        }

        r.read_exact(&mut word)?;
        let coll_type: CollType = match i32::from_le_bytes(word) {
            x if x == CollType::Convex as i32 => CollType::Convex,
            x if x == CollType::Concave as i32 => CollType::Concave,
            _ => panic!("Unknown coll type."),
        };

        r.read_exact(&mut word)?;
        let num_verts = u32::from_le_bytes(word);

        let count = (num_verts as usize)
            .checked_mul(3)
            .expect("Triangle count isn't a multiple of 9");

        let mut vbuf = Vec::new();
        for _ in 0..count {
            r.read_exact(&mut word)?;
            vbuf.push(f32::from_le_bytes(word));
        }

        Ok(Self {
            signature,
            version,
            coll_type,
            num_verts,
            vbuf,
        })
    }

    pub fn from_gltf<P: AsRef<Path>>(path: P, coll_type: CollType) -> Self {
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

        // TODO: remove dupe and unnecessary vertices if type is convex

        let num_verts = u32::try_from(vbuf.len() / 3).expect("vertices overflow");

        Self {
            signature: *SIGNATURE,
            version: VERSION,
            coll_type,
            num_verts,
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

    if let Some(mesh) = node.mesh() {
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
                    tris.extend(
                        indices
                            .as_chunks::<3>()
                            .0
                            .iter()
                            .map(|c| [c[1], c[0], c[2]]),
                    );
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
