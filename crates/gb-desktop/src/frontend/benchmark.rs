fn run_desktop_benchmark_suite(
    options: DesktopRunOptions,
    settings_store: DesktopSettingsStore,
    current_dir: PathBuf,
) -> Result<(), String> {
    let benchmark_path = options
        .benchmark_path
        .as_ref()
        .expect("benchmark suite runner requires a benchmark path");
    let benchmark_path = resolve_path(&current_dir, benchmark_path);
    let benchmark_cases =
        load_benchmark_cases(&benchmark_path).map_err(|error| error.to_string())?;

    for benchmark_case in benchmark_cases {
        let mut run_options = options.clone();
        run_options.benchmark_path = None;
        apply_benchmark_case_to_desktop_options(&mut run_options, &benchmark_case);
        run_desktop_prepared(
            run_options,
            settings_store.clone(),
            false,
            Some(benchmark_case),
            current_dir.clone(),
        )?;
    }

    Ok(())
}
