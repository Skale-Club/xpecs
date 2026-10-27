//! Windows power plans, through powercfg.

use crate::win;
use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::process::Command;

/// Template of the hidden "Ultimate Performance" plan.
const ULTIMATE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";

#[derive(Serialize)]
pub struct Plan {
    guid: String,
    name: String,
    active: bool,
}

fn powercfg(args: &[&str]) -> Option<String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new("powercfg.exe").args(args).creation_flags(CREATE_NO_WINDOW).output().ok()?;
    out.status.success().then(|| win::from_oem(&out.stdout))
}

fn is_guid(s: &str) -> bool {
    s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-') && s.matches('-').count() == 4
}

/// Lines look like "Power Scheme GUID: <guid>  (Balanced) *" in any language.
fn parse(text: &str) -> Vec<Plan> {
    text.lines()
        .filter_map(|line| {
            let guid = line.split_whitespace().find(|w| is_guid(w))?.to_lowercase();
            let name = match (line.find('('), line.rfind(')')) {
                (Some(a), Some(b)) if b > a => line[a + 1..b].trim().to_string(),
                _ => guid.clone(),
            };
            Some(Plan { guid, name, active: line.trim_end().ends_with('*') })
        })
        .collect()
}

pub fn list() -> Vec<Plan> {
    powercfg(&["/list"]).map(|t| parse(&t)).unwrap_or_default()
}

pub fn set_active(guid: &str) -> Result<(), String> {
    if !is_guid(guid) {
        return Err("bad plan id".into());
    }
    powercfg(&["/setactive", guid]).map(|_| ()).ok_or_else(|| "powercfg failed".into())
}

/// Adds a copy of the Ultimate Performance plan (hidden by default).
pub fn add_ultimate() -> Result<(), String> {
    powercfg(&["-duplicatescheme", ULTIMATE]).map(|_| ()).ok_or_else(|| "powercfg failed".into())
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_localized_output() {
        let text = "Esquemas de Energia Existentes (* Ativo)\n\
                    -----------------------------------\n\
                    GUID do Esquema de Energia: 381b4222-f694-41f0-9685-ff5bb260df2e  (Equilibrado) *\n\
                    GUID do Esquema de Energia: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (Alto desempenho)\n";
        let plans = parse(text);
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].name, "Equilibrado");
        assert!(plans[0].active && !plans[1].active);
        assert_eq!(plans[1].guid, "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c");
    }
}
