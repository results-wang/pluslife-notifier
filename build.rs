fn main() {
    // fontconfig on musl apparently has some leaky deps. Forcibly link them.
    if cfg!(target_env = "musl") {
        println!("cargo:rustc-link-arg=-lexpat");
        println!("cargo:rustc-link-arg=-lpng");
        println!("cargo:rustc-link-arg=-lz");
        println!("cargo:rustc-link-arg=-lbz2");
        println!("cargo:rustc-link-arg=-lbrotlidec");
    }
}
