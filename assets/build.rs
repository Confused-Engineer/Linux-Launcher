extern crate winres;


fn main() {
  if cfg!(target_os = "windows") {
    static_vcruntime::metabuild();
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/favicon.ico"); // Replace this with the filename of your .ico file.
    res.compile().unwrap();
  }
}

