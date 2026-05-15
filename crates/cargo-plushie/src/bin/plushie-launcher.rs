fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let version = args.iter().any(|arg| arg == "--version");
    let json = args.iter().any(|arg| arg == "--json");

    if version {
        return cargo_plushie::print_tool_version("plushie-launcher", json);
    }

    eprintln!(
        "plushie-launcher is a reusable package-launcher runtime. \
         Package assembly support is not wired to this binary yet. \
         Run with --version or --version --json to inspect its identity."
    );
    Ok(())
}
