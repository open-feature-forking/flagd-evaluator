use std::io::Result;

fn main() -> Result<()> {
    // Compile the protobuf schema
    prost_build::compile_protos(&["proto/evaluation.proto"], &["proto/"])?;
    Ok(())
}
