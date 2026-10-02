fn main() {
    glib_build_tools::compile_resources(
        &["data"],
        "data/app.drey.Vocalis.data.gresource.xml",
        "app.drey.Vocalis.data.gresource",
    );
}
