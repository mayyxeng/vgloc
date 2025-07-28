use crate::app::Config;
use git2::build::RepoBuilder;
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use std::env;
use tempfile::TempDir;

use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::JoinHandle;

use tokei::{LanguageType, Languages};

#[derive(Debug)]
pub struct CodeStats {
    pub language: LanguageType,
    pub files: usize,
    pub code: usize,
    pub comments: usize,
    pub blanks: usize,
}
#[derive(Debug)]
pub struct CommitReport {
    pub commit_date: i64,
    pub commit_hash: String,
    pub stats: Vec<CodeStats>,
}
pub enum LoaderCommand {
    Config(Config),
    Die,
}
pub enum LoaderData {
    CommitReport(CommitReport),
    FetchProgress,
}

pub struct RepositoryLoader {
    // repository: Option<RepositoryHandle>,
    command_tx: SyncSender<LoaderCommand>,
    data_rx: Receiver<Result<LoaderData, LoaderError>>,
    worker: JoinHandle<()>,
}
struct RepositoryHandle {
    temp_dir: TempDir,
    repository: Repository,
}

enum LoaderQuiteCause {
    Die,
    Finished,
}

impl RepositoryLoader {
    pub fn new() -> Self {
        log::debug!("Creating a data loader");
        let (command_tx, command_rx) = mpsc::sync_channel::<LoaderCommand>(1);
        let (data_tx, data_rx) = mpsc::sync_channel::<Result<LoaderData, LoaderError>>(1024);
        let worker = std::thread::spawn(move || {
            loader_main(command_rx, data_tx);
        });
        Self {
            command_tx,
            data_rx,
            worker,
        }
    }

    pub fn update_config(&self, config: Config) {
        self.command_tx.send(LoaderCommand::Config(config)).unwrap();
        log::debug!("Requested repository walk");
    }
    pub fn try_recv(&self) -> Option<Result<LoaderData, LoaderError>> {
        match self.data_rx.try_recv() {
            Ok(value) => Some(value),
            Err(TryRecvError::Disconnected) => panic!("thread is dead"),
            _ => None,
        }
    }
}

fn initialize(config: &Config) -> Result<RepositoryHandle, LoaderError> {
    let temp_dir = tempfile::tempdir_in(".")
        .map_err(|_| LoaderError::Other("Failed to create temp dir".to_owned()))?;
    let repo_path = temp_dir.path();
    let repository = if is_local_repo(&config.repo_url) {
        copy_local_repo(Path::new(&config.repo_url), repo_path)
    } else {
        clone_remote(&config.repo_url, repo_path)
    }?;
    let handle = RepositoryHandle {
        repository,
        temp_dir,
    };
    Ok(handle)
}

fn loader_main(
    command_rx: Receiver<LoaderCommand>,
    data_tx: SyncSender<Result<LoaderData, LoaderError>>,
) {
    log::debug!("Loader thread started");
    loop {
        match loader_loop(&command_rx, &data_tx) {
            Ok(LoaderQuiteCause::Die) => break,
            Ok(LoaderQuiteCause::Finished) => continue,
            Err(e) => {
                log::error!("Got error: {e}");
                data_tx.send(Err(e)).unwrap();
                continue;
            }
        }
    }
    log::debug!("Loader thread ended");
}
fn loader_loop(
    command_rx: &Receiver<LoaderCommand>,
    data_tx: &SyncSender<Result<LoaderData, LoaderError>>,
) -> Result<LoaderQuiteCause, LoaderError> {
    let config = match command_rx.recv().unwrap() {
        LoaderCommand::Die => return Ok(LoaderQuiteCause::Die),
        LoaderCommand::Config(config) => config,
    };
    log::debug!(
        "Repository walk begin with url: {} branch: {} depth: {}",
        config.repo_url,
        config.repo_branch,
        config.depth
    );
    let RepositoryHandle {
        temp_dir,
        repository,
    } = initialize(&config)?;
    let repo_path = temp_dir.path();
    log::debug!("Repository cloned to {}", repo_path.to_string_lossy());
    let mut revwalk = repository.revwalk().expect("Failed to get revwalk");
    let obj = repository
        .revparse_single(&format!("refs/remotes/origin/{}", config.repo_branch))
        .or_else(|_| repository.revparse_single(&format!("refs/heads/{}", config.repo_branch)))
        .map_err(|e| LoaderError::Other(format!("{e}")))?;
    revwalk.push(obj.id()).unwrap();

    for maybe_commit_id in revwalk.take(config.depth) {
        let commit_id = maybe_commit_id.map_err(|e| LoaderError::Other(format!("{e}")))?;
        let commit = repository.find_commit(commit_id).unwrap();
        let tree = commit.tree().unwrap();

        repository.checkout_tree(tree.as_object(), None).unwrap();
        repository.set_head_detached(commit.id()).unwrap();

        let config = tokei::Config::default();
        let mut stats = Languages::new();
        stats.get_statistics(&[repo_path.to_path_buf()], &[], &config);

        log::debug!("commit date: {}", commit.time().seconds());
        log::debug!("commit hash: {}", commit.id());
        let report = CommitReport {
            commit_date: commit.time().seconds(),
            commit_hash: commit.id().to_string(),
            stats: stats
                .iter()
                .map(|(l, d)| CodeStats {
                    language: *l,
                    files: d.reports.len(),
                    code: d.code,
                    blanks: d.blanks,
                    comments: d.comments,
                })
                .collect(),
        };
        log::debug!("Report: {report:?}");
        data_tx.send(Ok(LoaderData::CommitReport(report))).unwrap();
    }
    log::debug!("Finished processing repository");
    Ok(LoaderQuiteCause::Finished)
}

pub enum LoaderError {
    Git(git2::Error),
    Other(String),
}
impl From<git2::Error> for LoaderError {
    fn from(value: git2::Error) -> Self {
        Self::Git(value)
    }
}
impl std::error::Error for LoaderError {}
impl std::fmt::Debug for LoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git(e) => write!(f, "Loader git error: {e:?}"),
            Self::Other(s) => write!(f, "Loader error: {s:?}"),
        }
    }
}
impl std::fmt::Display for LoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git(e) => write!(f, "Loader git error: {e}"),
            Self::Other(s) => write!(f, "Loader error: {s}"),
        }
    }
}

fn is_local_repo(url: &str) -> bool {
    Path::new(url).exists()
}

fn clone_remote(repo_url: &str, target: &Path) -> Result<Repository, LoaderError> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.is_ssh_key() {
            Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        } else if allowed_types.is_user_pass_plaintext() {
            let user = env::var("GIT_USERNAME").unwrap_or_else(|_| "git".into());
            let pass = env::var("GIT_PASSWORD").expect("Set GIT_PASSWORD or GIT_TOKEN env var");
            Cred::userpass_plaintext(&user, &pass)
        } else {
            Err(git2::Error::from_str("Unsupported credential type"))
        }
    });

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_opts);
    log::debug!(
        "Cloning remote repository: {repo_url} into {}",
        target.to_string_lossy()
    );
    builder.clone(repo_url, target).map_err(From::from)
}

fn copy_local_repo(src: &Path, dest: &Path) -> Result<Repository, LoaderError> {
    fs_extra::dir::copy(
        src,
        dest,
        &fs_extra::dir::CopyOptions::new().content_only(true),
    )
    .map_err(|_| LoaderError::Other("Failed to copy local git repo".to_owned()))?;

    Repository::open(dest).map_err(From::from)
}
