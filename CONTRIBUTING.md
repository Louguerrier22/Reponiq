# Contributing to Reponiq

Thanks for helping make repository management better. This project is at an early MVP stage; smaller, well-tested changes are preferred over broad refactors.

## Workflow

1. Discuss significant feature changes in an issue before implementation.
2. Make a focused branch (`feat/...`, `fix/...`, `docs/...`).
3. Keep commits scoped and explain the user-visible impact.
4. Run the available frontend and Rust checks listed in [development setup](docs/DEVELOPMENT.md).
5. Open a PR with a concise problem statement, screenshots for UI changes, test results, and any security implications.

## PR checklist

- [ ] No credentials, `.env` files, or personal data committed
- [ ] New Git operations require explicit user intent and validation
- [ ] No unexpected shell execution or token-bearing arbitrary URLs
- [ ] Added tests for behavioral changes (or explained why not)
- [ ] Docs and roadmap updated for changed functionality
- [ ] Dependencies are justified and licenses reviewed

## Coding conventions

Prefer idiomatic Rust and typed TypeScript. Keep UI effects light, ensure error messages are actionable, and preserve local-first/offline behavior. Git mutations should be testable against disposable repositories. Do not change scope merely to rewrite working code.

## Community standards

Be respectful, specific, and constructive. Report security concerns privately (see [security](docs/SECURITY.md)).
