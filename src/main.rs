use clap::Parser;
use collcruncher::types::coll::Coll;
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
}

fn main() {
    let args = Args::parse();
    let out_path = args
        .output
        .unwrap_or_else(|| args.input.with_extension("coll"));

    let coll = Coll::from_gltf(&args.input);

    let mut w = BufWriter::new(File::create(&out_path).unwrap());
    coll.serialize(&mut w).unwrap();
    w.flush().unwrap();

    println!(
        "wrote {} triangles to {}",
        coll.num_tris,
        out_path.display()
    );
}
