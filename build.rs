fn main() {
    println!("cargo:rerun-if-changed=assets/machineglass.rc");
    println!("cargo:rerun-if-changed=assets/machineglass.ico");
    embed_resource::compile("assets/machineglass.rc", embed_resource::NONE)
        .manifest_required()
        .expect("MachineGlass Windows icon resource must compile");
}
