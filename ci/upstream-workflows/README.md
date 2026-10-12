# Archived WezTerm workflows

These workflows are preserved for reference outside `.github/workflows`, so
GitHub Actions does not run them. TermViAI currently validates Windows x86_64.
The archive includes upstream platform builds, nightly/tag publishing,
documentation, dependency updates, and issue maintenance.

`python3 ci/generate-workflows.py` regenerates platform workflows here, not in
the active workflows directory. Restore individual workflows deliberately when
their platform, credentials, packaging, and repository assumptions are supported.

Active CI consists of Windows GUI compilation and core tests on relevant main
branch pushes and pull requests, plus Rust formatting checks. Windows CI also
supports manual dispatch. There are no scheduled builds or automatic releases.
The uploaded GUI executable is a build artifact; it is not a portable release
package and does not bundle runtime DLLs.
