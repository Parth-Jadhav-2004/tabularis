//! MCP diagnostics must never format message bodies from drivers or dependencies.
use log::{Log, Metadata, Record};

struct AiLogger {
    level: log::LevelFilter,
}

impl Log for AiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{}", diagnostic_metadata(record));
        }
    }

    fn flush(&self) {}
}

fn diagnostic_metadata(record: &Record) -> String {
    // module_path_static is compile-time metadata, unlike dynamic targets/args.
    format!(
        "[MCP] [{}] Diagnostic from {}",
        record.level(),
        record.module_path_static().unwrap_or("unknown module")
    )
}

pub fn init(level: log::LevelFilter) -> Result<(), &'static str> {
    log::set_boxed_logger(Box::new(AiLogger { level }))
        .map_err(|_| "Could not initialize private MCP diagnostics")?;
    log::set_max_level(level);
    Ok(())
}

#[cfg(test)]
mod tests;
