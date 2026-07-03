use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchArgs {
    pub port: u16,
    pub plugin_uuid: String,
    pub register_event: String,
    pub info: String,
}

impl LaunchArgs {
    pub fn parse() -> Result<Self> {
        let argv: Vec<String> = std::env::args().skip(1).collect();
        Self::from_argv(&argv)
    }

    pub fn from_argv(argv: &[String]) -> Result<Self> {
        let value = |flag: &str| -> Option<&str> {
            argv.iter()
                .position(|a| a == flag)
                .and_then(|i| argv.get(i + 1))
                .map(String::as_str)
        };

        let port_raw = value("-port").context("отсутствует аргумент -port")?;
        let port: u16 = port_raw
            .parse()
            .with_context(|| format!("-port не является числом u16: {port_raw:?}"))?;
        let plugin_uuid = value("-pluginUUID").context("отсутствует аргумент -pluginUUID")?;
        let register_event = value("-registerEvent").context("отсутствует аргумент -registerEvent")?;
        let info = value("-info").context("отсутствует аргумент -info")?;

        if plugin_uuid.is_empty() || register_event.is_empty() {
            bail!("-pluginUUID и -registerEvent не должны быть пустыми");
        }

        Ok(Self {
            port,
            plugin_uuid: plugin_uuid.to_owned(),
            register_event: register_event.to_owned(),
            info: info.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_single_dash_long_flags() {
        let v = argv(&[
            "-port", "28196",
            "-pluginUUID", "ABC123",
            "-registerEvent", "registerPlugin",
            "-info", r#"{"application":{"platform":"mac"}}"#,
        ]);
        let parsed = LaunchArgs::from_argv(&v).unwrap();
        assert_eq!(parsed.port, 28196);
        assert_eq!(parsed.plugin_uuid, "ABC123");
        assert_eq!(parsed.register_event, "registerPlugin");
        assert_eq!(parsed.info, r#"{"application":{"platform":"mac"}}"#);
    }

    #[test]
    fn order_independent() {
        let v = argv(&[
            "-info", "{}",
            "-registerEvent", "registerPlugin",
            "-port", "1",
            "-pluginUUID", "U",
        ]);
        let parsed = LaunchArgs::from_argv(&v).unwrap();
        assert_eq!(parsed.port, 1);
        assert_eq!(parsed.plugin_uuid, "U");
    }

    #[test]
    fn missing_required_flag_errors() {
        let v = argv(&["-port", "9", "-pluginUUID", "X", "-registerEvent", "Y"]);
        assert!(LaunchArgs::from_argv(&v).is_err());
    }

    #[test]
    fn non_numeric_port_errors() {
        let v = argv(&["-port", "abc", "-pluginUUID", "X", "-registerEvent", "Y", "-info", "{}"]);
        assert!(LaunchArgs::from_argv(&v).is_err());
    }
}
