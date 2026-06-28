use flate2::read::GzDecoder;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::paths;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldSourceKind {
    Local,
    GitHub,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedWorldSource {
    pub source: String,
    pub dir: PathBuf,
    pub kind: WorldSourceKind,
}

pub trait WorldSourceReporter: Sync {
    fn resolving(&self, _source: &str) {}
    fn using_cached_world(&self, _dir: &Path) {}
    fn downloading_github_world(&self, _source: &str, _dir: &Path) {}
    fn downloading_archive(&self, _url: &str) {}
    fn downloaded_file(&self, _path: &str, _bytes: u64) {}
    fn ready(&self, _dir: &Path) {}
}

#[derive(Debug, Default)]
pub struct NoopWorldSourceReporter;

impl WorldSourceReporter for NoopWorldSourceReporter {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitHubTreeSource {
    owner: String,
    repo: String,
    reference: String,
    subdir: String,
}

#[derive(Debug)]
pub enum WorldSourceError {
    LocalPathMissing(PathBuf),
    MissingWorldToml(PathBuf),
    UnsupportedUrl(String),
    InvalidGitHubUrl(String),
    Io(String),
    Http(String),
}

impl std::fmt::Display for WorldSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalPathMissing(path) => write!(f, "path does not exist: {}", path.display()),
            Self::MissingWorldToml(path) => {
                write!(f, "world directory must contain world.toml: {}", path.display())
            }
            Self::UnsupportedUrl(url) => write!(
                f,
                "unsupported world URL: {url}\nSupported remote worlds are GitHub folder URLs like https://github.com/<owner>/<repo>/tree/<ref>/<path>"
            ),
            Self::InvalidGitHubUrl(url) => write!(
                f,
                "invalid GitHub world URL: {url}\nExpected https://github.com/<owner>/<repo>/tree/<ref>/<path>"
            ),
            Self::Io(message) | Self::Http(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WorldSourceError {}

pub fn resolve_world_source(
    source: impl AsRef<str>,
) -> Result<ResolvedWorldSource, WorldSourceError> {
    resolve_world_source_with_reporter(source, &NoopWorldSourceReporter)
}

pub fn resolve_world_source_with_reporter(
    source: impl AsRef<str>,
    reporter: &dyn WorldSourceReporter,
) -> Result<ResolvedWorldSource, WorldSourceError> {
    let source = source.as_ref();
    reporter.resolving(source);
    if looks_like_url(source) {
        let github = parse_github_tree_url(source)?;
        let dir = resolve_github_tree(&github, source, reporter)?;
        require_world_toml(&dir)?;
        reporter.ready(&dir);
        return Ok(ResolvedWorldSource {
            source: source.to_string(),
            dir,
            kind: WorldSourceKind::GitHub,
        });
    }

    let path = PathBuf::from(source);
    let dir = std::fs::canonicalize(&path).map_err(|_| WorldSourceError::LocalPathMissing(path))?;
    require_world_toml(&dir)?;
    reporter.ready(&dir);
    Ok(ResolvedWorldSource {
        source: source.to_string(),
        dir,
        kind: WorldSourceKind::Local,
    })
}

fn require_world_toml(dir: &Path) -> Result<(), WorldSourceError> {
    if dir.join("world.toml").is_file() {
        Ok(())
    } else {
        Err(WorldSourceError::MissingWorldToml(dir.to_path_buf()))
    }
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn parse_github_tree_url(url: &str) -> Result<GitHubTreeSource, WorldSourceError> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .ok_or_else(|| WorldSourceError::UnsupportedUrl(url.to_string()))?;
    let parts: Vec<&str> = rest.trim_matches('/').split('/').collect();
    if parts.len() < 5 || parts[2] != "tree" {
        return Err(WorldSourceError::InvalidGitHubUrl(url.to_string()));
    }
    let owner = parts[0];
    let repo = parts[1];
    let reference = parts[3];
    let subdir = parts[4..].join("/");
    if owner.is_empty() || repo.is_empty() || reference.is_empty() || subdir.is_empty() {
        return Err(WorldSourceError::InvalidGitHubUrl(url.to_string()));
    }
    Ok(GitHubTreeSource {
        owner: owner.to_string(),
        repo: repo.to_string(),
        reference: reference.to_string(),
        subdir,
    })
}

fn resolve_github_tree(
    source: &GitHubTreeSource,
    original_source: &str,
    reporter: &dyn WorldSourceReporter,
) -> Result<PathBuf, WorldSourceError> {
    let dir = github_cache_dir(source)?;
    if dir.join("world.toml").is_file() {
        reporter.using_cached_world(&dir);
        return Ok(dir);
    }
    reporter.downloading_github_world(original_source, &dir);

    let tmp = dir.with_extension("tmp");
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|err| {
            WorldSourceError::Io(format!(
                "failed to clear temporary world cache {}: {err}",
                tmp.display()
            ))
        })?;
    }
    if let Some(parent) = tmp.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            WorldSourceError::Io(format!(
                "failed to create world cache directory {}: {err}",
                parent.display()
            ))
        })?;
    }
    std::fs::create_dir_all(&tmp).map_err(|err| {
        WorldSourceError::Io(format!(
            "failed to create temporary world cache {}: {err}",
            tmp.display()
        ))
    })?;

    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| WorldSourceError::Io(format!("failed to create async runtime: {err}")))?;
    let result = rt.block_on(download_github_tree_archive(source, &tmp, reporter));
    if let Err(err) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(err);
    }

    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|err| {
            WorldSourceError::Io(format!(
                "failed to replace cached world {}: {err}",
                dir.display()
            ))
        })?;
    }
    std::fs::rename(&tmp, &dir).map_err(|err| {
        WorldSourceError::Io(format!(
            "failed to move cached world from {} to {}: {err}",
            tmp.display(),
            dir.display()
        ))
    })?;
    Ok(dir)
}

fn github_cache_dir(source: &GitHubTreeSource) -> Result<PathBuf, WorldSourceError> {
    let mut dir = paths::world_cache_dir()
        .map_err(WorldSourceError::Io)?
        .join("github")
        .join(safe_component(&source.owner))
        .join(safe_component(&source.repo))
        .join(safe_component(&source.reference));
    for segment in source.subdir.split('/') {
        dir.push(safe_component(segment));
    }
    Ok(dir)
}

fn safe_component(value: &str) -> String {
    let safe: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "_".to_string()
    } else {
        safe
    }
}

async fn download_github_tree_archive(
    source: &GitHubTreeSource,
    target: &Path,
    reporter: &dyn WorldSourceReporter,
) -> Result<(), WorldSourceError> {
    let client = reqwest::Client::builder()
        .user_agent("clawblox-cli")
        .build()
        .map_err(|err| WorldSourceError::Http(format!("failed to create HTTP client: {err}")))?;
    let url = format!(
        "https://codeload.github.com/{}/{}/tar.gz/{}",
        source.owner, source.repo, source.reference
    );
    reporter.downloading_archive(&url);
    let response = client.get(&url).send().await.map_err(|err| {
        WorldSourceError::Http(format!("failed to download GitHub archive: {err}"))
    })?;
    if !response.status().is_success() {
        return Err(WorldSourceError::Http(format!(
            "GitHub returned {} for {}",
            response.status(),
            url
        )));
    }
    let archive = response
        .bytes()
        .await
        .map_err(|err| WorldSourceError::Http(format!("failed to read GitHub archive: {err}")))?;
    extract_world_from_github_archive(&archive, &source.subdir, target, reporter)
}

fn extract_world_from_github_archive(
    archive: &[u8],
    subdir: &str,
    target: &Path,
    reporter: &dyn WorldSourceReporter,
) -> Result<(), WorldSourceError> {
    std::fs::create_dir_all(target).map_err(|err| {
        WorldSourceError::Io(format!(
            "failed to create world cache directory {}: {err}",
            target.display()
        ))
    })?;

    let decoder = GzDecoder::new(archive);
    let mut archive = tar::Archive::new(decoder);
    let subdir = Path::new(subdir);
    let mut extracted_files = 0usize;
    for entry in archive.entries().map_err(|err| {
        WorldSourceError::Io(format!("failed to read GitHub archive entries: {err}"))
    })? {
        let mut entry = entry.map_err(|err| {
            WorldSourceError::Io(format!("failed to read GitHub archive entry: {err}"))
        })?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|err| {
            WorldSourceError::Io(format!("failed to read GitHub archive path: {err}"))
        })?.into_owned();
        let mut components = path.components();
        let _repo_root = components.next();
        let relative_repo_path = components.as_path().to_path_buf();
        let Ok(relative_world_path) = relative_repo_path.strip_prefix(subdir) else {
            continue;
        };
        if relative_world_path.as_os_str().is_empty() {
            continue;
        }
        let out_path = target.join(relative_world_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                WorldSourceError::Io(format!(
                    "failed to create world cache directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|err| {
            WorldSourceError::Io(format!(
                "failed to extract GitHub archive file {}: {err}",
                relative_repo_path.display()
            ))
        })?;
        std::fs::write(&out_path, &bytes).map_err(|err| {
            WorldSourceError::Io(format!(
                "failed to write cached GitHub file {}: {err}",
                out_path.display()
            ))
        })?;
        extracted_files += 1;
        reporter.downloaded_file(
            &relative_repo_path.display().to_string(),
            bytes.len() as u64,
        );
    }

    if extracted_files == 0 {
        return Err(WorldSourceError::MissingWorldToml(target.to_path_buf()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_tree_url() {
        let parsed = parse_github_tree_url(
            "https://github.com/nacloos/clawblox/tree/main/worlds/mesa-world",
        )
        .unwrap();
        assert_eq!(parsed.owner, "nacloos");
        assert_eq!(parsed.repo, "clawblox");
        assert_eq!(parsed.reference, "main");
        assert_eq!(parsed.subdir, "worlds/mesa-world");
    }

    #[test]
    fn rejects_non_github_urls() {
        assert!(matches!(
            parse_github_tree_url("https://example.com/world"),
            Err(WorldSourceError::UnsupportedUrl(_))
        ));
    }

    #[test]
    fn sanitizes_cache_components() {
        assert_eq!(safe_component("feature/worlds"), "feature_worlds");
        assert_eq!(safe_component("mesa-world"), "mesa-world");
    }

    #[test]
    fn resolves_local_world_dir() {
        let dir =
            std::env::temp_dir().join(format!("clawblox-world-source-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("world.toml"), "name = \"test\"\n").unwrap();

        let resolved = resolve_world_source(dir.to_string_lossy()).unwrap();
        assert_eq!(resolved.kind, WorldSourceKind::Local);
        assert!(resolved.dir.is_absolute());
        assert!(resolved.dir.join("world.toml").is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
