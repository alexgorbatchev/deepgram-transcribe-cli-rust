use cli_command_kit::{Mode, display_message};
use std::io::{self, Write};
pub fn text(value: &str) -> String {
    display_message(value)
        .replace(['\n', '\r', '\t'], " ")
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}
pub fn status(out: &mut dyn Write, mode: Mode, kind: &str, message: &str) -> io::Result<()> {
    writeln!(
        out,
        "{} {}",
        if mode == Mode::Agent {
            format!("{kind}:")
        } else {
            format!("[{kind}]")
        },
        text(message)
    )
}
pub fn fields(out: &mut dyn Write, mode: Mode, fields: Vec<(&str, String)>) -> io::Result<()> {
    for (key, value) in fields {
        writeln!(
            out,
            "{}: {}",
            if mode == Mode::Agent {
                key.into()
            } else {
                key.split('_')
                    .map(|s| {
                        let mut c = s.chars();
                        c.next().map_or(String::new(), |f| {
                            f.to_uppercase().collect::<String>() + c.as_str()
                        })
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            },
            text(&value)
        )?;
    }
    Ok(())
}
pub fn table(
    out: &mut dyn Write,
    mode: Mode,
    headers: &[&str],
    rows: Vec<Vec<String>>,
) -> io::Result<()> {
    let rows = rows
        .into_iter()
        .map(|row| row.into_iter().map(|v| text(&v)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    if mode == Mode::Agent {
        writeln!(out, "{}", headers.join("\t"))?;
        for row in rows {
            writeln!(out, "{}", row.join("\t"))?;
        }
    } else {
        let mut table = comfy_table::Table::new();
        let width = cli_command_kit::terminal_width();
        table
            .load_preset(comfy_table::presets::ASCII_FULL)
            .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
            .set_width(if width == 0 {
                100
            } else {
                width.min(u16::MAX as usize) as u16
            })
            .set_header(headers);
        for row in rows {
            table.add_row(row);
        }
        writeln!(out, "{table}")?;
    }
    Ok(())
}
