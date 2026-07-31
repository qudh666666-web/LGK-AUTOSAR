use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let mut port = None;
    let mut token_file = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--port" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--port requires a value"))?;
                port = Some(value.to_string_lossy().parse::<u16>()?);
            }
            "--token-file" => {
                token_file =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("--token-file requires a value")
                    })?));
            }
            other => anyhow::bail!("unsupported argument: {other}"),
        }
    }
    let port = port.unwrap_or(autosar_ecuc_bridge::app::DEFAULT_HOST_PORT);
    let token_file = token_file.unwrap_or_else(autosar_ecuc_bridge::app::token_file);
    autosar_ecuc_bridge::app::host::run(port, &token_file)
}
