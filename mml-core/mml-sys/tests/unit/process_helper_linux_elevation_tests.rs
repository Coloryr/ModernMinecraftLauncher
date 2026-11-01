use super::*;

#[test]
fn restores_user_home_after_pkexec_without_shell_interpolation() {
    let command = command(
        Path::new("/opt/my launcher"),
        &["a ' b".into()],
        HashMap::from([("HOME".into(), "/home/player".into())]),
        Path::new("/home/player/mml"),
    );
    let args: Vec<_> = command
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args[0], "/usr/bin/env");
    assert!(args.contains(&"--chdir=/home/player/mml".into()));
    assert!(args.contains(&"HOME=/home/player".into()));
    assert_eq!(&args[args.len() - 2..], ["/opt/my launcher", "a ' b"]);
}

