fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/icon.ico");
        res.set("ProductName", "Kairos");
        res.set("FileDescription", "Kairos — Tick-based orderflow platform");
        res.compile().expect("Failed to compile Windows resources");
    }
}
