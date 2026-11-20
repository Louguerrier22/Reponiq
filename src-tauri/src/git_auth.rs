use std::process::Command;

/// A command-local helper: never write the account token into a remote URL,
/// command-line argument, repository config, or the system credential helper.
pub(super) struct GithubCredentials {
    pub token: String,
    pub full_name: String,
}

pub(super) enum CloneAuthentication {
    PublicGithub { full_name: String },
    Github(GithubCredentials),
}

impl CloneAuthentication {
    pub fn configure(&self, command: &mut Command) {
        let full_name = match self {
            Self::PublicGithub { full_name } => full_name,
            Self::Github(credentials) => &credentials.full_name,
        };
        let remote = format!("https://github.com/{full_name}.git");
        // Public GitHub repositories must not depend on account permissions or
        // stale system credentials. Reset repository settings at the same URL
        // specificity as any saved overrides.
        command.args([
            "-c",
            "credential.helper=",
            "-c",
            "http.https://github.com/.extraHeader=",
            "-c",
            &format!("http.{remote}.extraHeader="),
            "-c",
            &format!("http.{remote}.cookieFile="),
            "-c",
            &format!("http.{remote}.saveCookies=false"),
            "-c",
            "core.askPass=",
        ]);
        command.env("GIT_ASKPASS", "").env("SSH_ASKPASS", "");
        if let Self::Github(credentials) = self {
            credentials.configure(command, &remote);
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
    fn configure(&self, command: &mut Command, remote: &str) {
        super::diagnostics::remember_secret(&self.token);
        // Authenticated transfers own their command-level configuration and
        // never write diagnostics or TLS session keys through inherited knobs.
        for name in [
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_PARAMETERS",
            "GIT_CURL_VERBOSE",
            "GIT_SSL_NO_VERIFY",
            "SSLKEYLOGFILE",
            "GIT_SSL_KEYLOGFILE",
        ] {
            command.env_remove(name);
        }
        for name in [
            "GIT_TRACE",
            "GIT_TRACE_PACKET",
            "GIT_TRACE_CURL",
            "GIT_TRACE2",
            "GIT_TRACE2_EVENT",
            "GIT_TRACE2_PERF",
        ] {
            command.env(name, "0");
        }
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
                "-c",
                &format!("http.{remote}.followRedirects=false"),
                "-c",
                "http.https://github.com/.sslVerify=true",
                "-c",
                &format!("http.{remote}.sslVerify=true"),
                "-c",
                if cfg!(windows) {
                    "core.hooksPath=NUL"
                } else {
                    "core.hooksPath=/dev/null"
                },
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
        let fixture = tempfile::tempdir().unwrap();
        let store = fixture.path().join("fixture-credential-store");
        let saved = "https://saved-user:stale-secret@github.com/owner/project.git\n";
        std::fs::write(&store, saved).unwrap();
        let mut command = Command::new(crate::git_binary().unwrap());
        command.current_dir(fixture.path()).args([
            "-c",
            "credential.helper=store --file=fixture-credential-store",
        ]);
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
        let output = child.wait_with_output().unwrap();
        assert_eq!(std::fs::read_to_string(store).unwrap(), saved);
        output
    }

    #[test]
    fn account_token_is_only_supplied_for_the_selected_https_repository() {
        let output = credential(
            "fill",
            "protocol=https\nhost=github.com\npath=owner/project.git\n\n",
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("username=x-access-token\npassword=fixture-secret"));
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
        CloneAuthentication::Github(GithubCredentials {
            token: "fixture-secret".into(),
            full_name: "owner/project".into(),
        })
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
        let authentication = CloneAuthentication::PublicGithub {
            full_name: "owner/project".into(),
        };
        assert_http_option(
            &authentication,
            "extraHeader",
            "Authorization: stale-secret",
            "",
        );
        assert_http_option(&authentication, "cookieFile", "fixture-cookies.txt", "");
        assert_http_option(&authentication, "saveCookies", "true", "false");
        let mut command = Command::new(crate::git_binary().unwrap());
        authentication.configure(&mut command);
        assert!(!command
            .get_envs()
            .any(|(key, _)| key == "REPONIQ_GIT_TOKEN"));
        let output = credential_with_auth(
            "fill",
            "protocol=https\nhost=github.com\npath=owner/project.git\n\n",
            authentication,
        );
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("stale-secret"));
    }

    #[test]
    fn private_github_clone_uses_repository_transport_policy() {
        let authentication = CloneAuthentication::Github(GithubCredentials {
            token: "fixture-secret".into(),
            full_name: "owner/project".into(),
        });
        assert_http_option(
            &authentication,
            "extraHeader",
            "Authorization: stale-secret",
            "",
        );
        assert_http_option(&authentication, "cookieFile", "fixture-cookies.txt", "");
        assert_http_option(&authentication, "saveCookies", "true", "false");
        assert_http_option(&authentication, "sslVerify", "false", "true");
        assert_http_option(&authentication, "followRedirects", "true", "false");
    }

    fn assert_http_option(
        authentication: &CloneAuthentication,
        option: &str,
        configured: &str,
        expected: &str,
    ) {
        let remote = "https://github.com/owner/project.git";
        let mut command = Command::new(crate::git_binary().unwrap());
        command.args(["-c", &format!("http.{remote}.{option}={configured}")]);
        authentication.configure(&mut command);
        let output = command
            .args([
                "config",
                "--get-urlmatch",
                &format!("http.{option}"),
                remote,
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    }
}
