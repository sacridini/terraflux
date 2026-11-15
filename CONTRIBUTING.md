# Contributing to Terraflux

Bug reports, feature requests, corrections to the documentation and code are welcome. The full guide is on the
website: **<https://sacridini.github.io/terraflux/about/contributing/>**. In short:

- **Report a bug** in the [issues](https://github.com/sacridini/terraflux/issues): what you did, what you expected,
  what happened, the version (`terraflux --version`, or Settings, About & updates), your operating system, the
  error text, and the model file (`.toml`), as small as you can make it.
- **Ask for a feature** in an issue: the problem first, then the reference (paper or software) of the method.
- **Improve the documentation**: every page of the [website](https://sacridini.github.io/terraflux/) has a
  "Suggest a change to this page" link. To build the site, see [website/README.md](website/README.md); the style is
  in [website/AUTHORING.md](website/AUTHORING.md).
- **Build and test the code**: see [Building from source](README.md#building-from-source). Test the crate you
  changed, with the cores limited (`cargo test -p terraflux-ops -j 4 -- --test-threads=4`).
- **Commit messages** follow the history: `feat(scope): ...`, `fix: ...`, `docs(website): ...`, `test: ...`.
- **Security problems** are not reported in public: see [SECURITY.md](SECURITY.md).

Everyone who takes part follows the [Code of Conduct](CODE_OF_CONDUCT.md). By sending a contribution, you agree that
it is released under the project's license, GPL-3.0-or-later ([LICENSE](LICENSE)).
