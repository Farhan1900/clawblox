# Releasing Clawblox

The install command downloads the version named by
<https://releases.clawblox.com/latest>. Changes on `main` reach users after a
versioned release is published.

## Publish a version

1. Update the root package version in `Cargo.toml` and its `Cargo.lock` entry.
2. Open a pull request. `Check release builds` builds all six supported platforms
   without publishing. Review the changes and wait for those builds to pass.
3. Merge the version bump into `main`. `Release version` creates the tag and calls
   the release workflow directly. Tags pushed with `GITHUB_TOKEN` do not trigger
   another workflow, so the direct call is required.
4. The release workflow checks the tag and package versions, builds from one
   commit, verifies the uploads, and then updates `latest`. The manifest records
   the source commit as well as the platform checksums.
5. Check that the installer selects the new version and that `clawblox --version`
   reports it on a clean install.

Every push to `main` checks whether the package version has already been tagged.
If the tag points to an older commit, the workflow reports how many commits remain
unreleased in a warning and its summary. It does not silently publish those
commits under an existing version.

## Retry a failed release

Rerun the failed workflow, or run the `Release` workflow manually from `main`
with `ref` set to the existing version tag and `publish` enabled. Leave `publish`
disabled to only check the builds. Manual tag pushes also run the release
workflow. Publishing requires a version newer than the current `latest`; never
move a released tag or overwrite an existing installed version.
