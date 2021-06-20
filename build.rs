fn main() {
    tonic_build::compile_protos("proto/ecosystem.proto")
        .unwrap_or_else(|e| panic!("Failed to compile ecosystem.proto {:?}", e));
    tonic_build::compile_protos("proto/observer.proto")
        .unwrap_or_else(|e| panic!("Failed to compile observer.proto {:?}", e));
}
