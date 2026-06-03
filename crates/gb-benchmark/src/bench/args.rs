use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BenchAction {
    ShowHelp,
    Sample,
    NormalizeCase {
        case_dir: PathBuf,
    },
    RewriteRomDir {
        case_dir: PathBuf,
        rom_dir: PathBuf,
    },
    GenerateCases {
        case_dir: PathBuf,
        rom_dir: PathBuf,
        template_path: Option<PathBuf>,
    },
    Run(BenchRunOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BenchRunOptions {
    pub(super) case_dir: Option<PathBuf>,
    pub(super) single_test: Option<PathBuf>,
    pub(super) include_cli: bool,
}

pub fn bench_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo rom-bench --sample\n",
        "  cargo rom-bench <case-dir> [--rom-dir <rom-dir>]\n",
        "  cargo rom-bench <case-dir> --normalize-case\n",
        "  cargo rom-bench <case-dir> --rom-dir <rom-dir> --generate-cases [--template <case.toml>]\n",
        "  cargo rom-bench [<case-dir>] [--gb-cli] --test <case.toml>\n",
        "\n",
        "Arguments:\n",
        "  <case-dir>        Directory containing benchmark case *.toml files; optional with --test.\n",
        "\n",
        "Options:\n",
        "  --sample          Create test/bench/game.toml if missing.\n",
        "  --rom-dir <dir>   Rewrite rom = \"...\" in <case-dir>/*.toml preserving each ROM basename.\n",
        "  --normalize-case  Rename <case-dir>/*.toml from each case's ROM filename stem.\n",
        "  --generate-cases  Generate normalized cases for every *.gb and *.gbc ROM under --rom-dir.\n",
        "  --template <path>  Use a benchmark case TOML template with --generate-cases.\n",
        "  --gb-cli          Run gb-cli in addition to the default gb-desktop benchmark.\n",
        "  --test <path>     Run one benchmark case; without <case-dir>, infer it from this file.\n",
        "  -h, --help        Show this help.\n",
        "\n",
        "Outputs are written to test/bench/. By default only gb-desktop runs.\n",
    )
}

pub(super) fn parse_bench_arguments<I, S>(arguments: I) -> Result<BenchAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut action_sample = false;
    let mut rom_dir = None;
    let mut include_cli = false;
    let mut single_test = None;
    let mut case_dir = None;
    let mut normalize_case = false;
    let mut generate_cases = false;
    let mut template_path = None;
    let mut arguments = arguments.into_iter();

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--sample" => action_sample = true,
            "--rom-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--rom-dir requires a value".to_string());
                };
                rom_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--normalize-case" => normalize_case = true,
            "--generate-cases" => generate_cases = true,
            "--template" => {
                let Some(value) = arguments.next() else {
                    return Err("--template requires a value".to_string());
                };
                template_path = Some(PathBuf::from(value.as_ref()));
            }
            "--gb-cli" => include_cli = true,
            "--test" => {
                let Some(value) = arguments.next() else {
                    return Err("--test requires a value".to_string());
                };
                single_test = Some(PathBuf::from(value.as_ref()));
            }
            "-h" | "--help" => return Ok(BenchAction::ShowHelp),
            value if value.starts_with('-') => {
                return Err(format!("unknown option {value}"));
            }
            value => {
                if case_dir.is_some() {
                    return Err(format!("unexpected argument {value}"));
                }
                case_dir = Some(PathBuf::from(value));
            }
        }
    }

    if action_sample {
        if case_dir.is_some()
            || rom_dir.is_some()
            || single_test.is_some()
            || include_cli
            || normalize_case
            || generate_cases
            || template_path.is_some()
        {
            return Err("--sample cannot be combined with benchmark run options".to_string());
        }
        return Ok(BenchAction::Sample);
    }

    if normalize_case
        && (rom_dir.is_some()
            || single_test.is_some()
            || include_cli
            || generate_cases
            || template_path.is_some())
    {
        return Err("--normalize-case cannot be combined with other benchmark actions".to_string());
    }

    if template_path.is_some() && !generate_cases {
        return Err("--template requires --generate-cases".to_string());
    }

    if generate_cases && rom_dir.is_none() {
        return Err("--generate-cases requires --rom-dir".to_string());
    }

    if generate_cases && (single_test.is_some() || include_cli) {
        return Err("--generate-cases cannot be combined with benchmark run options".to_string());
    }

    if rom_dir.is_some() && !generate_cases && (single_test.is_some() || include_cli) {
        return Err("--rom-dir cannot be combined with benchmark run options".to_string());
    }

    if normalize_case {
        let Some(case_dir) = case_dir else {
            return Err("<case-dir> is required".to_string());
        };
        return Ok(BenchAction::NormalizeCase { case_dir });
    }

    if generate_cases {
        let Some(case_dir) = case_dir else {
            return Err("<case-dir> is required".to_string());
        };
        return Ok(BenchAction::GenerateCases {
            case_dir,
            rom_dir: rom_dir.expect("rom dir was validated above"),
            template_path,
        });
    }

    if let Some(rom_dir) = rom_dir {
        let Some(case_dir) = case_dir else {
            return Err("<case-dir> is required".to_string());
        };
        return Ok(BenchAction::RewriteRomDir { case_dir, rom_dir });
    }

    if case_dir.is_none() && single_test.is_none() {
        return Err("<case-dir> is required".to_string());
    }

    Ok(BenchAction::Run(BenchRunOptions {
        case_dir,
        single_test,
        include_cli,
    }))
}
