use super::*;

#[test]
fn diagnostics_do_not_include_sql_parameters_or_dynamic_targets() {
    let record = Record::builder()
        .level(log::Level::Debug)
        .target("private-target-marker")
        .module_path_static(Some("tokio_postgres::query"))
        .args(format_args!(
            "SELECT 'private-query-marker' params='private-parameter-marker'"
        ))
        .build();
    let diagnostic = diagnostic_metadata(&record);
    assert!(diagnostic.contains("tokio_postgres::query"));
    assert!(!diagnostic.contains("private-") && !diagnostic.contains("SELECT"));
}

#[test]
fn sensitive_argument_display_is_never_evaluated() {
    struct Sensitive;
    impl std::fmt::Display for Sensitive {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("Sensitive message must not be formatted");
        }
    }
    let sensitive = Sensitive;
    let args = format_args!("{}", sensitive);
    let record = Record::builder()
        .level(log::Level::Error)
        .args(args)
        .build();
    assert_eq!(
        diagnostic_metadata(&record),
        "[MCP] [ERROR] Diagnostic from unknown module"
    );
}

#[test]
fn private_logger_respects_the_requested_level() {
    let logger = AiLogger {
        level: log::LevelFilter::Warn,
    };
    assert!(logger.enabled(&Metadata::builder().level(log::Level::Error).build()));
    assert!(!logger.enabled(&Metadata::builder().level(log::Level::Debug).build()));
}
