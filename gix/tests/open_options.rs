use std::fs;

use gix_testtools::{Env, Result, tempfile};

#[test]
fn preset_system_config_paths_avoid_running_git() -> Result {
    let temp = tempfile::tempdir()?;
    let git_dir = temp.path().join("repo.git");
    fs::create_dir_all(git_dir.join("objects"))?;
    fs::create_dir_all(git_dir.join("refs"))?;
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n")?;
    fs::write(
        git_dir.join("config"),
        "[core]\n\trepositoryFormatVersion = 0\n\tbare = true\n",
    )?;

    let installation_config = temp.path().join("installation.gitconfig");
    fs::write(&installation_config, "[preset]\n\tinstallation = configured\n")?;
    let system_config = temp.path().join("system.gitconfig");
    fs::write(&system_config, "[preset]\n\tsystem = configured\n")?;

    let trace = temp.path().join("git.trace");
    let _env = Env::new().set("GIT_TRACE", trace.to_string_lossy());
    let mut options = gix::open::Options::isolated()
        .git_installation_config_path(&installation_config)
        .system_config_path(&system_config);
    options.permissions.config.git_binary = true;
    options.permissions.config.system = true;

    let repo = gix::open_opts(&git_dir, options)?;
    assert_eq!(
        repo.config_snapshot()
            .string("preset.installation")
            .expect("installation configuration was loaded"),
        "configured"
    );
    assert_eq!(
        repo.config_snapshot()
            .string("preset.system")
            .expect("system configuration was loaded"),
        "configured"
    );

    let _no_system = Env::new().set("GIT_CONFIG_NOSYSTEM", "1");
    let mut options = gix::open::Options::isolated()
        .git_installation_config_path(installation_config)
        .system_config_path(system_config);
    options.permissions.config.git_binary = true;
    options.permissions.config.system = true;
    options.permissions.env.git_prefix = gix::sec::Permission::Allow;
    let repo = gix::open_opts(git_dir, options)?;
    assert!(
        repo.config_snapshot().string("preset.installation").is_none(),
        "GIT_CONFIG_NOSYSTEM must disable preset installation configuration"
    );
    assert!(
        repo.config_snapshot().string("preset.system").is_none(),
        "GIT_CONFIG_NOSYSTEM must disable preset system configuration"
    );
    assert!(!trace.exists(), "presetting both paths must avoid launching Git");
    Ok(())
}
