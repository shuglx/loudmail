fn main() {
    // alert-sound 目录被 include_dir 嵌入，新增/修改音频时也需要重新编译
    println!("cargo:rerun-if-changed=../alert-sound");
    tauri_build::build()
}
