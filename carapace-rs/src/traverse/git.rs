//! Parent-directory lookups (port of Go `pkg/traverse`).

use crate::context::Context;

/// Location of the `.git` folder (Go `GitDir`): the `GIT_DIR` environment
/// variable when set, otherwise the work tree's `.git` directory.
pub fn git_dir(context: &Context) -> Result<String, String> {
    if let Some(dir) = context.lookup_env("GIT_DIR") {
        return Ok(dir.to_owned());
    }
    Ok(format!("{}/.git", git_work_tree(context)?))
}

/// Location of the root of the working directory for a non-bare repository
/// (Go `GitWorkTree`): the `GIT_WORK_TREE` environment variable when set,
/// otherwise the first parent directory containing `.git`.
pub fn git_work_tree(context: &Context) -> Result<String, String> {
    if let Some(dir) = context.lookup_env("GIT_WORK_TREE") {
        return Ok(dir.to_owned());
    }
    parent(context, &[".git"])
}

/// The first parent directory (starting at the context directory) that
/// contains any of `names` (Go `Parent`).
pub fn parent(context: &Context, names: &[&str]) -> Result<String, String> {
    let mut dir = context.abs("")?;
    loop {
        for name in names {
            let target = format!(
                "{}/{}",
                dir.trim_end_matches('/'),
                name.trim_end_matches('/')
            );
            if std::fs::metadata(&target).is_ok() {
                return Ok(dir.trim_end_matches('/').to_owned());
            }
        }
        match parent_of(&dir) {
            Some(parent) => dir = parent,
            None => {
                return Err(format!(
                    "could not find parent directory containing any of: {names:?}"
                ));
            }
        }
    }
}

fn parent_of(dir: &str) -> Option<String> {
    let parent = std::path::Path::new(dir)
        .parent()?
        .to_string_lossy()
        .into_owned();
    (parent != dir).then_some(parent)
}
