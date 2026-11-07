use std::process::Command;

/// A command-local helper: never write the account token into a remote URL,
/// command-line argument, repository config, or the system credential helper.
pub(super) struct GithubCredentials {
    pub token: String,
    pub full_name: String,
}

pub(super) enum CloneAuthentication {
    PublicGithub,
    Github(GithubCredentials),
}

impl CloneAuthentication {
    pub fn configure(&self, command: &mut Command) {
        // Public GitHub repositories must not depend on account permissions or
        // stale system credentials/Authorization headers.
        command.args([
            "-c",
            "credential.helper=",
            "-c",
            "http.https://github.com/.extraHeader=",
            "-c",
            "core.askPass=",
        ]);
        command.env("GIT_ASKPASS", "").env("SSH_ASKPASS", "");
        if let Self::Github(credentials) = self {
            credentials.configure(command);
        }
    }

    pub fn uses_account(&self) -> bool {
        matches!(self, Self::Github(_))
    }
}

const HELPER: &str = r#"!f() {
    [ "$1" = get ] || exit 0
    protocol= host= path=
    while IFS='=' read -r key value; do
        [ -n "$key" ] || break
        case "$key" in
            protocol) protocol=$value ;;
            host) host=$value ;;
            path) path=$value ;;
        esac
    done
    if [ "$protocol" = https ] && [ "$host" = github.com ] && [ "$path" = "$REPONIQ_GIT_PATH" ]; then
        printf '%s\n' 'username=x-access-token' "password=$REPONIQ_GIT_TOKEN"
    fi
}; f"#;

impl GithubCredentials {
    pub fn configure(&self, command: &mut Command) {
        super::diagnostics::remember_secret(&self.token);
        command
            .args([
                "-c",
                "credential.helper=",
                "-c",
                &format!("credential.helper={HELPER}"),
                "-c",
                "credential.useHttpPath=true",
                "-c",
                "http.followRedirects=false",
            ])
            .env("REPONIQ_GIT_TOKEN", &self.token)
            .env("REPONIQ_GIT_PATH", format!("{}.git", self.full_name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::Stdio;

    fn credential(operation: &str, input: &str) -> std::process::Output {
        credential_with_auth(
            operation,
            input,
            CloneAuthentication::Github(GithubCredentials {
                token: "fixture-secret".into(),
                full_name: "owner/project".into(),
            }),
        )
    }

    fn credential_with_auth(
        operation: &str,
        input: &str,
        authentication: CloneAuthentication,
    ) -> std::process::Output {
        let mut command = Command::new(crate::git_binary().unwrap());
        command.args(["-c", "credential.helper=!echo password=stale-secret"]);
        authentication.configure(&mut command);
        let mut child = command
            .args(["credential", operation])
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    #[test]
    fn account_token_is_only_supplied_for_the_selected_https_repository() {
        let output = credential(
            "fill",
            "protocol=https\nhost=github.com\npath=owner/project.git\n\n",
        );
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("password=fixture-secret"));
        for input in [
            "protocol=http\nhost=github.com\npath=owner/project.git\n\n",
            "protocol=https\nhost=other.example\npath=owner/project.git\n\n",
            "protocol=https\nhost=github.com\npath=owner/other.git\n\n",
        ] {
            let output = credential("fill", input);
            assert!(!output.status.success());
            assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture-secret"));
        }
    }

    #[test]
    fn credentials_are_not_in_arguments_or_persistent_configuration() {
        let mut command = Command::new("git");
        GithubCredentials {
            token: "fixture-secret".into(),
            full_name: "owner/project".into(),
        }
        .configure(&mut command);
        assert!(!command
            .get_args()
            .any(|arg| arg.to_string_lossy().contains("fixture-secret")));
        for operation in ["approve", "reject"] {
            let output = credential(operation, "protocol=https\nhost=github.com\npath=owner/project.git\nusername=x-access-token\npassword=fixture-secret\n\n");
            assert!(output.status.success());
            assert!(output.stdout.is_empty());
        }
    }

    #[test]
    fn public_github_clone_clears_stale_authentication() {
        let mut command = Command::new(crate::git_binary().unwrap());
        command.args([
            "-c",
            "credential.helper=!echo password=stale-secret",
            "-c",
            "http.https://github.com/.extraHeader=Authorization: stale-secret",
        ]);
        CloneAuthentication::PublicGithub.configure(&mut command);
        let output = command
            .args([
                "config",
                "--get-urlmatch",
                "http.extraHeader",
                "https://github.com/owner/project.git",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().trim().is_empty());
        assert!(!command
            .get_envs()
            .any(|(key, _)| key == "REPONIQ_GIT_TOKEN"));
        let output = credential_with_auth(
            "fill",
            "protocol=https\nhost=github.com\npath=owner/project.git\n\n",
            CloneAuthentication::PublicGithub,
        );
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("stale-secret"));
    }
}
