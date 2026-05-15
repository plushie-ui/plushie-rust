fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let version = args.iter().any(|arg| arg == "--version");
    let json = args.iter().any(|arg| arg == "--json");

    if version {
        return cargo_plushie::print_tool_version("plushie-launcher", json);
    }

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    let mut manifest_path = None;
    let mut postcheck = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    anyhow::bail!("--manifest requires a path");
                };
                manifest_path = Some(std::path::PathBuf::from(value));
            }
            "--postcheck" => {
                postcheck = true;
            }
            other => {
                anyhow::bail!(
                    "unknown plushie-launcher argument `{other}`; run with --help for usage"
                );
            }
        }
        index += 1;
    }

    let postcheck = postcheck || std::env::var_os("PLUSHIE_PACKAGE_POSTCHECK").is_some();
    let code = if let Some(manifest_path) = manifest_path {
        if postcheck {
            cargo_plushie::package_runtime::postcheck_external_package(&manifest_path)?
        } else {
            cargo_plushie::package_runtime::run_external_package(&manifest_path)?
        }
    } else {
        let executable = std::env::current_exe()?;
        let code = if postcheck {
            cargo_plushie::package_runtime::postcheck_embedded_package(&executable)?
        } else {
            cargo_plushie::package_runtime::run_embedded_package(&executable)?
        };
        code.ok_or_else(|| {
            anyhow::anyhow!(
                "no embedded Plushie package found; run with --manifest PATH or package with `plushie package portable`"
            )
        })?
    };
    std::process::exit(i32::from(code));
}

fn print_help() {
    println!(
        "Usage: plushie-launcher [--manifest PATH] [--postcheck]\n\n\
         Options:\n\
           --manifest PATH  Run a Plushie package manifest and sibling payload archive\n\
                            When omitted, run package data embedded in this executable\n\
           --postcheck      Validate extraction and diagnostics without starting the host\n\
           --version        Print human-readable identity\n\
           --version --json Print machine-readable identity"
    );
}
