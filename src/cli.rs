use serde_json::Map;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Options {
    pub json: bool,
    pub color: crate::output::ColorMode,
    pub app: String,
    pub env: String,
    pub build_type: String,
    pub build_key: String,
    pub filter: String,
    pub output: String,
    pub search: String,
    pub app_type: String,
    pub revision: Option<i32>,
    pub offset: Option<i64>,
    pub limit: i64,
    pub interval: Duration,
    pub timeout: Duration,
    pub max_parallel: usize,
    pub max_depth: i32,
    pub no_wait: bool,
    pub continue_on_error: bool,
    pub skip_dependencies: bool,
    pub all_producers: bool,
    pub updates: Map<String, serde_json::Value>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            json: false,
            color: crate::output::ColorMode::Auto,
            app: String::new(),
            env: String::new(),
            build_type: "Release".to_string(),
            build_key: String::new(),
            filter: String::new(),
            output: String::new(),
            search: String::new(),
            app_type: String::new(),
            revision: None,
            offset: None,
            limit: 100,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(1800),
            max_parallel: 3,
            max_depth: -1,
            no_wait: false,
            continue_on_error: false,
            skip_dependencies: false,
            all_producers: false,
            updates: Map::new(),
        }
    }
}

pub struct CommandDefinition {
    pub name: &'static str,
    pub short: &'static str,
    pub positional: &'static str,
    pub group: &'static str,
}

pub const COMMANDS: &[CommandDefinition] = &[
    CommandDefinition {
        name: "discover",
        short: "Show the OAuth discovery document (issuer, endpoints, scopes).",
        positional: "",
        group: "auth",
    },
    CommandDefinition {
        name: "login",
        short: "Save credentials in ~/.odc/config.json (prompts for client secret).",
        positional: "<tenant-url> <client-id>",
        group: "auth",
    },
    CommandDefinition {
        name: "list-environments",
        short: "List environments in the tenant.",
        positional: "",
        group: "inspect",
    },
    CommandDefinition {
        name: "list-apps",
        short: "List apps in the tenant, optionally filtered by type or name.",
        positional: "",
        group: "inspect",
    },
    CommandDefinition {
        name: "list-deployed-apps",
        short: "List deployed apps, optionally filtered by environment, name or key.",
        positional: "",
        group: "inspect",
    },
    CommandDefinition {
        name: "get-app",
        short: "Retrieve app metadata.",
        positional: "[app-name-or-key]",
        group: "inspect",
    },
    CommandDefinition {
        name: "latest-revision",
        short: "Print the latest revision number of an app.",
        positional: "[app-name-or-key]",
        group: "inspect",
    },
    CommandDefinition {
        name: "list-revisions",
        short: "List all revisions of an app.",
        positional: "[app-name-or-key]",
        group: "inspect",
    },
    CommandDefinition {
        name: "get-revision",
        short: "Retrieve a specific app revision.",
        positional: "[app-name-or-key]",
        group: "inspect",
    },
    CommandDefinition {
        name: "producer-graph",
        short: "Render an app's producer dependency graph as Mermaid.",
        positional: "[app-name-or-key]",
        group: "inspect",
    },
    CommandDefinition {
        name: "download-source-code",
        short: "Download the OML source code of an app revision.",
        positional: "[app-name-or-key]",
        group: "inspect",
    },
    CommandDefinition {
        name: "upload-source-code",
        short: "Upload an OML/XIF file, creating a new asset or revision.",
        positional: "<oml-file>",
        group: "inspect",
    },
    CommandDefinition {
        name: "validate",
        short: "Validate that an app can be deployed to an environment.",
        positional: "",
        group: "inspect",
    },
    CommandDefinition {
        name: "analyze-deployment",
        short: "Analyze the impact of deploying an app revision.",
        positional: "[app-name-or-key]",
        group: "analyze",
    },
    CommandDefinition {
        name: "analyze-deletion",
        short: "Analyze the impact of deleting an app.",
        positional: "[app-name-or-key]",
        group: "analyze",
    },
    CommandDefinition {
        name: "deploy",
        short: "Deploy an app to an environment.",
        positional: "",
        group: "deploy",
    },
    CommandDefinition {
        name: "undeploy",
        short: "Undeploy an app from an environment.",
        positional: "",
        group: "deploy",
    },
    CommandDefinition {
        name: "delete-app",
        short: "Delete an app.",
        positional: "",
        group: "deploy",
    },
    CommandDefinition {
        name: "batch-deploy",
        short: "Deploy multiple apps listed in a file.",
        positional: "<apps-file>",
        group: "deploy",
    },
    CommandDefinition {
        name: "batch-undeploy",
        short: "Undeploy multiple apps listed in a file.",
        positional: "<apps-file>",
        group: "deploy",
    },
    CommandDefinition {
        name: "batch-delete",
        short: "Delete multiple apps listed in a file.",
        positional: "<apps-file>",
        group: "deploy",
    },
    CommandDefinition {
        name: "dangerous-batch-undeploy-all",
        short: "Undeploy all apps from an environment.",
        positional: "",
        group: "deploy",
    },
    CommandDefinition {
        name: "get-user",
        short: "Retrieve a user's details.",
        positional: "<user-key-or-email>",
        group: "users",
    },
    CommandDefinition {
        name: "update-user",
        short: "Update a user's name, active status, or photo URL.",
        positional: "<user-key-or-email>",
        group: "users",
    },
    CommandDefinition {
        name: "grant-role",
        short: "Grant an application role to a user.",
        positional: "<user-key-or-email> <role-name-or-key>",
        group: "users",
    },
    CommandDefinition {
        name: "revoke-role",
        short: "Revoke an application role from a user.",
        positional: "<user-key-or-email> <role-name-or-key>",
        group: "users",
    },
    CommandDefinition {
        name: "internal-build",
        short: "Start a build for an app revision.",
        positional: "",
        group: "internal",
    },
    CommandDefinition {
        name: "internal-publish",
        short: "Publish a build to an environment.",
        positional: "",
        group: "internal",
    },
    CommandDefinition {
        name: "internal-deploy",
        short: "Deploy an existing build to an environment.",
        positional: "",
        group: "internal",
    },
];

pub fn parse_color(s: &str) -> anyhow::Result<crate::output::ColorMode> {
    match s {
        "auto" => Ok(crate::output::ColorMode::Auto),
        "always" => Ok(crate::output::ColorMode::Always),
        "never" => Ok(crate::output::ColorMode::Never),
        _ => Err(anyhow::anyhow!("--color must be auto, always, or never")),
    }
}

pub fn find_command(name: &str) -> Option<&'static CommandDefinition> {
    COMMANDS.iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_options_default() {
        let opts = Options::default();
        assert_eq!(opts.build_type, "Release");
        assert_eq!(opts.interval, Duration::from_secs(10));
        assert_eq!(opts.timeout, Duration::from_secs(1800));
        assert_eq!(opts.max_parallel, 3);
    }

    #[test]
    fn test_parse_color() {
        assert!(parse_color("auto").is_ok());
        assert!(parse_color("always").is_ok());
        assert!(parse_color("never").is_ok());
        assert!(parse_color("invalid").is_err());
    }

    #[test]
    fn test_find_command() {
        assert!(find_command("login").is_some());
        assert!(find_command("deploy").is_some());
        assert!(find_command("nonexistent").is_none());
    }

    #[test]
    fn test_all_commands_defined() {
        assert!(!COMMANDS.is_empty());
        assert!(find_command("login").is_some());
        assert!(find_command("dangerous-batch-undeploy-all").is_some());
    }
}
