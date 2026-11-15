# Security policy

## Supported versions

Only the **latest release** of Terraflux gets security fixes. A fix goes into the next release; it is not
back-ported to older ones. The latest release is on the
[releases page](https://github.com/sacridini/terraflux/releases/latest), and Terraflux tells you when a newer one
exists (Settings, About & updates).

The Python library [terraflux-py](https://github.com/sacridini/terraflux-py) is a separate project with its own
policy: report its problems in its own repository, the same way.

## Reporting a vulnerability

Please do **not** open a public issue for a security problem.

Report it privately with GitHub: open the **Security** tab of the
[repository](https://github.com/sacridini/terraflux), choose **Report a vulnerability**, and describe:

- what you found and where (the version from `terraflux --version`, the operating system);
- the steps to reproduce it, with a small model or file that shows it when you can;
- what you think an attacker could do with it.

## What to expect

- An acknowledgement within about a week. Terraflux has one maintainer, so please be patient.
- A first answer on whether it is a vulnerability, and how serious it looks.
- A fix in the next release when it is one, and a note in the [changelog](CHANGELOG.md). If you want to be named
  there, say so in your report.
- Please keep the problem private until a fixed release is published.

What Terraflux sends over the network is described on
[Privacy and network access](https://sacridini.github.io/terraflux/about/privacy/).
