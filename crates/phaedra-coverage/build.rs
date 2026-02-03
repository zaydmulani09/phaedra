fn main() {
    cc::Build::new()
        .file("sancov_rt/sancov_rt.c")
        .opt_level(2)
        .compile("sancov_rt");

    println!("cargo:rerun-if-changed=sancov_rt/sancov_rt.c");
}
