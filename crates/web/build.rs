fn main() {
    // Re-embed the SPA whenever the Vite build output changes.
    println!("cargo:rerun-if-changed=../../web/dist");
}
