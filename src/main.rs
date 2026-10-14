use clap::Parser;
use collcruncher::types::coll::Coll;
use collcruncher::types::coll::CollType;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    /// Source filepath; a gltf model
    input: PathBuf,

    /// Output filepath [default: source file with different extension]
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Make concave collider (by default it's convex)
    #[arg(short, long)]
    concave: bool,
}

fn main() {
    let args = Args::parse();
    let out_path = args
        .output
        .unwrap_or_else(|| args.input.with_extension("coll"));

    let coll_type = if args.concave {
        CollType::Concave
    } else {
        CollType::Convex
    };

    let coll = Coll::from_gltf(&args.input, coll_type);

    let mut w = BufWriter::new(File::create(&out_path).unwrap());
    coll.serialize(&mut w).unwrap();
    w.flush().unwrap();

    println!(
        "wrote {} triangles to {}",
        coll.num_verts,
        out_path.display()
    );
}
