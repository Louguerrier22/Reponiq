use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

const MAX_BYTES: usize = 64 * 1024;
const COLORS: &[&str] = &[
    "background",
    "foreground",
    "card",
    "card-foreground",
    "popover",
    "popover-foreground",
    "primary",
    "primary-foreground",
    "secondary",
    "secondary-foreground",
    "muted",
    "muted-foreground",
    "accent",
    "accent-foreground",
    "border",
    "input",
    "ring",
    "destructive",
    "sidebar",
    "sidebar-foreground",
    "sidebar-primary",
    "sidebar-primary-foreground",
    "sidebar-accent",
    "sidebar-accent-foreground",
    "sidebar-border",
    "sidebar-ring",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistryTheme {
    name: String,
    light: BTreeMap<String, String>,
    dark: BTreeMap<String, String>,
}

fn number(channel: &str) -> bool {
    channel
        .strip_suffix('%')
        .or_else(|| channel.strip_suffix("deg"))
        .unwrap_or(channel)
        .parse::<f64>()
        .is_ok_and(f64::is_finite)
}

fn color(value: &str) -> Option<String> {
    if value.len() > 64 {
        return None;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return (matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_owned());
    }
    let literal = if let Some((function, arguments)) = value.split_once('(') {
        let function = match function {
            "oklch" => "oklch",
            "rgb" | "rgba" => "rgb",
            "hsl" | "hsla" => "hsl",
            _ => return None,
        };
        let arguments = arguments.strip_suffix(')')?;
        let (components, alpha) = arguments
            .split_once('/')
            .map_or((arguments, None), |(a, b)| (a, Some(b.trim())));
        let mut channels = components
            .split(|character: char| character.is_ascii_whitespace() || character == ',')
            .filter(|channel| !channel.is_empty())
            .collect::<Vec<_>>();
        let alpha = if alpha.is_none()
            && channels.len() == 4
            && arguments.contains(',')
            && function != "oklch"
        {
            channels.pop()
        } else {
            alpha
        };
        if channels.len() != 3
            || !channels.iter().all(|channel| number(channel))
            || alpha.is_some_and(|value| !number(value) || value.ends_with("deg"))
        {
            return None;
        }
        let components = channels.join(" ");
        match alpha {
            Some(value) => format!("{function}({components} / {value})"),
            None => format!("{function}({components})"),
        }
    } else {
        let channels = value.split_ascii_whitespace().collect::<Vec<_>>();
        if channels.len() != 3
            || !channels[1].ends_with('%')
            || !channels[2].ends_with('%')
            || !channels.iter().all(|channel| number(channel))
        {
            return None;
        }
        format!("hsl({})", channels.join(" "))
    };
    Some(literal)
}

fn radius(value: &str) -> Option<String> {
    if value == "0" {
        return Some(value.to_owned());
    }
    if value.len() > 16 {
        return None;
    }
    let (amount, maximum) = value
        .strip_suffix("rem")
        .map(|v| (v, 2.5))
        .or_else(|| value.strip_suffix("px").map(|v| (v, 40.0)))?;
    let amount = amount.parse::<f64>().ok()?;
    (0.0..=maximum).contains(&amount).then(|| value.to_owned())
}

fn check_palette(tokens: &mut BTreeMap<String, String>) -> Result<(), String> {
    if tokens.len() > COLORS.len() + 1
        || !["background", "foreground", "primary", "primary-foreground"]
            .iter()
            .all(|key| tokens.contains_key(*key))
    {
        return Err(
            "The theme needs background, text, and primary colors for light and dark mode.".into(),
        );
    }
    for (key, value) in tokens {
        *value = if key == "radius" {
            radius(value.trim())
        } else if COLORS.contains(&key.as_str()) {
            color(value.trim())
        } else {
            None
        }
        .ok_or("The theme contains an unsupported color or radius.")?;
    }
    Ok(())
}

impl RegistryTheme {
    pub(super) fn validate(&mut self) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.chars().count() > 64
            || self.name.chars().any(char::is_control)
        {
            return Err("The theme name is invalid.".into());
        }
        check_palette(&mut self.light)?;
        check_palette(&mut self.dark)
    }
}

fn theme_slug(path: &str) -> Option<&str> {
    let slug = path.strip_prefix("/r/themes/")?.strip_suffix(".json")?;
    (!slug.is_empty()
        && slug.len() <= 64
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
    .then_some(slug)
}

fn parse_theme(bytes: &[u8], url: &reqwest::Url) -> Result<RegistryTheme, String> {
    if bytes.len() > MAX_BYTES {
        return Err("The theme file is too large.".into());
    }
    let item: Value =
        serde_json::from_slice(bytes).map_err(|_| "The link did not return registry JSON.")?;
    let official_palette = url.host_str() == Some("ui.shadcn.com")
        && theme_slug(url.path()).is_some()
        && item["type"].is_null()
        && item["name"].as_str() == theme_slug(url.path());
    if !official_palette
        && !matches!(
            item["type"].as_str(),
            Some("registry:theme" | "registry:style")
        )
    {
        return Err("Choose a shadcn registry theme or style JSON link.".into());
    }
    let vars = &item["cssVars"];
    let palette = |mode: &str| {
        COLORS
            .iter()
            .copied()
            .chain(["radius"])
            .filter_map(|key| {
                vars[mode][key]
                    .as_str()
                    .or_else(|| {
                        if key == "radius" {
                            vars["theme"][key].as_str()
                        } else {
                            None
                        }
                    })
                    .map(|value| (key.to_owned(), value.to_owned()))
            })
            .collect()
    };
    let mut output = RegistryTheme {
        name: item["title"]
            .as_str()
            .or_else(|| official_palette.then(|| item["label"].as_str()).flatten())
            .or_else(|| item["name"].as_str())
            .unwrap_or("Imported theme")
            .trim()
            .to_owned(),
        light: palette("light"),
        dark: palette("dark"),
    };
    output.validate()?;
    Ok(output)
}

fn registry_url(input: &str) -> Result<reqwest::Url, String> {
    let error = "Use a public shadcn theme JSON link or a GitHub link to the JSON file.";
    if input.len() > 2048 {
        return Err(error.into());
    }
    let mut url = reqwest::Url::parse(input.trim()).map_err(|_| error)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || !url.path().ends_with(".json")
    {
        return Err(error.into());
    }
    if url.host_str() == Some("github.com") {
        let mut parts: Vec<_> = url.path().split('/').collect();
        if parts.len() < 6 || parts[3] != "blob" {
            return Err(error.into());
        }
        parts.remove(3);
        let path = parts.join("/");
        url.set_host(Some("raw.githubusercontent.com"))
            .map_err(|_| error)?;
        url.set_path(&path);
    }
    let tweakcn_theme = url.host_str() == Some("tweakcn.com")
        && theme_slug(url.path()).is_some()
        && theme_slug(url.path()) != Some("registry");
    if !tweakcn_theme
        && !matches!(
            url.host_str(),
            Some(
                "ui.shadcn.com"
                    | "shadcn-svelte.com"
                    | "www.shadcn-svelte.com"
                    | "raw.githubusercontent.com"
            )
        )
    {
        return Err(error.into());
    }
    url.set_fragment(None);
    Ok(url)
}

#[tauri::command]
pub(super) async fn preview_registry_theme(url: String) -> Result<RegistryTheme, String> {
    let url = registry_url(&url)?;
    let message = "Could not download the theme JSON.";
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| message)?;
    let mut response = client.get(url.clone()).send().await.map_err(|_| message)?;
    if !response.status().is_success() {
        return Err(message.into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BYTES as u64)
    {
        return Err("The theme file is too large.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| message)? {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err("The theme file is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_theme(&bytes, &url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_json_link_uses_the_public_raw_file() {
        let link = registry_url("https://github.com/example/themes/blob/main/blue.json").unwrap();
        assert_eq!(
            link.as_str(),
            "https://raw.githubusercontent.com/example/themes/main/blue.json"
        );
        assert!(registry_url("https://github.com/example/themes").is_err());
        assert!(registry_url("https://example.com/blue.json").is_err());
    }

    #[test]
    fn shadcn_theme_uses_only_supported_tokens() {
        let light = serde_json::json!({"background":"oklch(0.99 0 0)","foreground":"#14233a","primary":"215 100% 28%","primary-foreground":"#fff","font-sans":"Example Font"});
        let item = serde_json::json!({"type":"registry:theme","name":"fixture","cssVars":{"light":light,"dark":light}});
        let url = registry_url("https://ui.shadcn.com/example.json").unwrap();
        let parsed = parse_theme(&serde_json::to_vec(&item).unwrap(), &url).unwrap();
        assert_eq!(parsed.light["primary"], "hsl(215 100% 28%)");
        assert!(!parsed.light.contains_key("font-sans"));
        assert!(color("var(--brand)").is_none());
        assert_eq!(
            color("oklch(1 0 0 / 10%)").as_deref(),
            Some("oklch(1 0 0 / 10%)")
        );
    }

    #[test]
    fn official_palettes_are_only_accepted_from_their_catalog() {
        let colors = serde_json::json!({"background":"0 0% 100%","foreground":"0 0% 10%","primary":"0 0% 9%","primary-foreground":"0 0% 98%"});
        let item = serde_json::json!({"name":"neutral","label":"Neutral","cssVars":{"light":colors,"dark":colors}});
        let bytes = serde_json::to_vec(&item).unwrap();
        let official = registry_url("https://ui.shadcn.com/r/themes/neutral.json").unwrap();
        assert_eq!(parse_theme(&bytes, &official).unwrap().name, "Neutral");
        let other =
            registry_url("https://raw.githubusercontent.com/example/themes/main/neutral.json")
                .unwrap();
        assert!(parse_theme(&bytes, &other).is_err());
        assert!(registry_url("https://tweakcn.com/r/themes/modern-minimal.json").is_ok());
        assert!(registry_url("https://tweakcn.com/r/themes/registry.json").is_err());
        assert!(registry_url("https://tweakcn.com/elsewhere.json").is_err());
    }
}
