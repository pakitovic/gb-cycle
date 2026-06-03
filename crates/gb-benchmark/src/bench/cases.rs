use serde::Deserialize;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::BenchmarkModel;

use super::paths::{
    bench_output_dir, canonicalize_lossy, case_files, io_error, relative_display, rom_files,
};

const DEFAULT_SAMPLE_NAME: &str = "game.toml";
const DEFAULT_TEMPLATE: &str = r#"version = 1
id = "game"
rom = "/roms/game.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
palette = "grey"
screenshot = true
stats = true

[[run]]
id = "idle-40"
duration_seconds = 40

[[run]]
id = "start-a-120"
duration_seconds = 120

[[run.input]]
frame = 30
button = "start"
hold_frames = 8
repeat_every_frames = 60

[[run.input]]
frame = 60
button = "a"
hold_frames = 8
repeat_every_frames = 60
"#;

pub(super) fn write_sample_case<W>(workspace_root: &Path, output: &mut W) -> Result<(), String>
where
    W: Write,
{
    let output_dir = bench_output_dir(workspace_root);
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "failed to create benchmark output directory {}: {error}",
            output_dir.display()
        )
    })?;
    let sample = output_dir.join(DEFAULT_SAMPLE_NAME);
    if sample.exists() {
        writeln!(output, "sample already exists: {}", sample.display()).map_err(io_error)
    } else {
        fs::write(&sample, DEFAULT_TEMPLATE)
            .map_err(|error| format!("failed to write sample {}: {error}", sample.display()))?;
        writeln!(output, "wrote {}", sample.display()).map_err(io_error)
    }
}

pub(super) fn rewrite_rom_dir<W>(
    case_dir: &Path,
    rom_dir: &Path,
    output: &mut W,
) -> Result<(), String>
where
    W: Write,
{
    let cases = case_files(case_dir)?;
    if cases.is_empty() {
        return Err(format!(
            "no benchmark cases found in {}",
            case_dir.display()
        ));
    }

    let mut updated = 0;
    for case_path in cases {
        let text = fs::read_to_string(&case_path)
            .map_err(|error| format!("failed to read {}: {error}", case_path.display()))?;
        let Some(rom) = top_level_string_value(&text, "rom") else {
            writeln!(
                output,
                "warning: no rom = entry found in {}",
                relative_display(&case_path, case_dir)
            )
            .map_err(io_error)?;
            continue;
        };
        let basename = portable_file_name(&rom);
        if basename.is_empty() {
            writeln!(
                output,
                "warning: {} has an empty ROM basename; skipped",
                case_path.display()
            )
            .map_err(io_error)?;
            continue;
        }
        let next_rom = rom_dir.join(basename);
        let (next_text, changed) = replace_top_level_string_value(
            &text,
            "rom",
            &next_rom.display().to_string(),
            InsertMissing::No,
        );
        if changed {
            fs::write(&case_path, next_text)
                .map_err(|error| format!("failed to write {}: {error}", case_path.display()))?;
            updated += 1;
            writeln!(output, "updated {}", relative_display(&case_path, case_dir))
                .map_err(io_error)?;
        }
    }

    writeln!(output, "updated {updated} benchmark case(s)").map_err(io_error)
}

pub(super) fn normalize_cases<W>(case_dir: &Path, output: &mut W) -> Result<(), String>
where
    W: Write,
{
    let cases = case_files(case_dir)?;
    if cases.is_empty() {
        return Err(format!(
            "no benchmark cases found in {}",
            case_dir.display()
        ));
    }

    let mut renamed = 0;
    let mut unchanged = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for case_path in cases {
        let text = fs::read_to_string(&case_path)
            .map_err(|error| format!("failed to read {}: {error}", case_path.display()))?;
        let Some(rom) = top_level_string_value(&text, "rom") else {
            writeln!(
                output,
                "warning: no top-level rom = entry found in {}; skipped",
                case_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>")
            )
            .map_err(io_error)?;
            skipped += 1;
            continue;
        };
        let target_name = normalized_case_name(&rom);
        let target_path = case_dir.join(&target_name);
        if case_path.file_name() == Some(OsStr::new(&target_name)) {
            unchanged += 1;
            writeln!(output, "unchanged {target_name}").map_err(io_error)?;
            continue;
        }
        if target_path.exists() {
            errors.push(format!(
                "cannot rename {} to {}; target already exists",
                case_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>"),
                target_name
            ));
            continue;
        }
        let source_name = case_path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("<unknown>")
            .to_string();
        fs::rename(&case_path, &target_path).map_err(|error| {
            format!(
                "failed to rename {} to {}: {error}",
                case_path.display(),
                target_path.display()
            )
        })?;
        renamed += 1;
        writeln!(output, "renamed {source_name} -> {target_name}").map_err(io_error)?;
    }

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    writeln!(
        output,
        "renamed {renamed}, unchanged {unchanged}, skipped {skipped} benchmark case(s)"
    )
    .map_err(io_error)
}

pub(super) fn generate_benchmark_cases<W>(
    case_dir: &Path,
    rom_dir: &Path,
    template_path: Option<&Path>,
    output: &mut W,
) -> Result<(), String>
where
    W: Write,
{
    let template_text = if let Some(template_path) = template_path {
        fs::read_to_string(template_path).map_err(|error| {
            format!(
                "failed to read template {}: {error}",
                template_path.display()
            )
        })?
    } else {
        DEFAULT_TEMPLATE.to_string()
    };

    let roms = rom_files(rom_dir)?;
    if roms.is_empty() {
        return Err(format!(
            "no .gb or .gbc ROMs found in {}",
            rom_dir.display()
        ));
    }

    let mut targets: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    let mut errors = Vec::new();
    for rom_path in roms {
        let stem = rom_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("game");
        let target_path = case_dir.join(format!("{stem}.toml"));
        if let Some((_, previous)) = targets.iter().find(|(target, _)| target == &target_path) {
            errors.push(format!(
                "ROMs {} and {} both normalize to {}",
                previous.display(),
                rom_path.display(),
                target_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>")
            ));
        }
        targets.push((target_path, rom_path));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    targets.sort_by(|left, right| {
        left.0
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .cmp(
                &right
                    .0
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
            )
    });

    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    for (target_path, rom_path) in targets {
        let rendered = render_case_from_template(&template_text, &rom_path)?;
        if target_path.exists() {
            let current = fs::read_to_string(&target_path)
                .map_err(|error| format!("failed to read {}: {error}", target_path.display()))?;
            if current == rendered {
                unchanged += 1;
                writeln!(
                    output,
                    "unchanged {}",
                    target_path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("<unknown>")
                )
                .map_err(io_error)?;
            } else {
                fs::write(&target_path, rendered).map_err(|error| {
                    format!("failed to write {}: {error}", target_path.display())
                })?;
                updated += 1;
                writeln!(
                    output,
                    "updated {}",
                    target_path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("<unknown>")
                )
                .map_err(io_error)?;
            }
        } else {
            fs::write(&target_path, rendered)
                .map_err(|error| format!("failed to write {}: {error}", target_path.display()))?;
            created += 1;
            writeln!(
                output,
                "wrote {}",
                target_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>")
            )
            .map_err(io_error)?;
        }
    }

    writeln!(
        output,
        "created {created}, updated {updated}, unchanged {unchanged} benchmark case(s)"
    )
    .map_err(io_error)
}

fn render_case_from_template(template: &str, rom_path: &Path) -> Result<String, String> {
    let absolute_rom = canonicalize_lossy(rom_path)?;
    let id = safe_id(
        rom_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("game"),
    );
    let model = model_for_rom(rom_path)?;
    let (text, _) = replace_top_level_string_value(template, "id", &id, InsertMissing::Yes);
    let (text, _) = replace_top_level_string_value(
        &text,
        "rom",
        &absolute_rom.display().to_string(),
        InsertMissing::Yes,
    );
    let (mut text, _) =
        replace_top_level_string_value(&text, "model", model.as_str(), InsertMissing::Yes);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn replace_top_level_string_value(
    text: &str,
    key: &str,
    value: &str,
    insert_missing: InsertMissing,
) -> (String, bool) {
    let mut output = String::new();
    let mut changed = false;
    let mut found = false;
    let mut in_top_level = true;
    let mut inserted = false;
    for line in text.split_inclusive('\n') {
        let (body, newline) = split_line_newline(line);
        let trimmed = body.trim_start();
        if trimmed.starts_with('[') {
            if in_top_level && !found && insert_missing == InsertMissing::Yes {
                output.push_str(&format!("{key} = {}\n", toml_string(value)));
                inserted = true;
                found = true;
                changed = true;
            }
            in_top_level = false;
            output.push_str(line);
            continue;
        }
        if in_top_level && !found && top_level_assignment_key(body) == Some(key) {
            let comment = line_comment(body).unwrap_or_default();
            output.push_str(&format!("{key} = {}{comment}{newline}", toml_string(value)));
            found = true;
            changed = true;
        } else {
            output.push_str(line);
        }
    }
    if !found && !inserted && insert_missing == InsertMissing::Yes {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&format!("{key} = {}\n", toml_string(value)));
        changed = true;
    }
    (output, changed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertMissing {
    No,
    Yes,
}

fn top_level_string_value(text: &str, key: &str) -> Option<String> {
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            break;
        }
        if top_level_assignment_key(raw_line) == Some(key) {
            let value = raw_line.split_once('=')?.1;
            return parse_toml_string(value);
        }
    }
    None
}

fn top_level_assignment_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let (key, _) = trimmed.split_once('=')?;
    let key = key.trim();
    if !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        Some(key)
    } else {
        None
    }
}

fn parse_toml_string(value: &str) -> Option<String> {
    let value = value.split('#').next().unwrap_or_default().trim();
    if value.is_empty() {
        return None;
    }
    #[derive(Deserialize)]
    struct InlineString {
        value: String,
    }
    toml::from_str::<InlineString>(&format!("value = {value}"))
        .ok()
        .map(|parsed| parsed.value)
}

fn line_comment(body: &str) -> Option<&str> {
    body.find('#').map(|index| &body[index..])
}

fn split_line_newline(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix('\n') {
        if let Some(body) = body.strip_suffix('\r') {
            (body, "\r\n")
        } else {
            (body, "\n")
        }
    } else {
        (line, "")
    }
}

fn toml_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn model_for_rom(path: &Path) -> Result<BenchmarkModel, String> {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
    {
        Some(extension) if extension == "gb" => Ok(BenchmarkModel::Dmg),
        Some(extension) if extension == "gbc" => Ok(BenchmarkModel::Cgb),
        _ => Err(format!("unsupported ROM suffix in {}", path.display())),
    }
}

fn safe_id(stem: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('-');
            last_was_separator = true;
        }
    }
    while slug.ends_with('-') || slug.ends_with('_') {
        slug.pop();
    }
    if slug.is_empty() {
        "game".to_string()
    } else {
        slug
    }
}

pub(super) fn portable_file_name(value: &str) -> &str {
    value.rsplit(['/', '\\']).next().unwrap_or(value)
}

fn normalized_case_name(rom: &str) -> String {
    let basename = portable_file_name(rom);
    let stem = basename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(basename);
    format!("{stem}.toml")
}
