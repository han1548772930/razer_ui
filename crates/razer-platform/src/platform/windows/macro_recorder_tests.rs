use super::Hooks;

#[test]
fn failed_mouse_install_releases_the_keyboard_without_publishing_started() {
    let mut hooks = Hooks::<u8>::new();
    let mut released = Vec::new();
    let error = hooks
        .install(
            false,
            |keyboard| {
                if keyboard {
                    Ok(13)
                } else {
                    anyhow::bail!("mouse install failed")
                }
            },
            |handle| {
                released.push(handle);
                Ok(())
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("mouse install failed"));
    assert_eq!(released, [13]);
    assert!(hooks.keyboard.is_none() && hooks.mouse.is_none());
}

#[test]
fn failed_partial_cleanup_retains_handle_and_blocks_a_new_install() {
    let mut hooks = Hooks::<u8>::new();
    let error = hooks
        .install(
            false,
            |keyboard| {
                if keyboard {
                    Ok(13)
                } else {
                    anyhow::bail!("mouse install failed")
                }
            },
            |_| anyhow::bail!("unhook failed"),
        )
        .unwrap_err();
    assert!(error.to_string().contains("partial hook cleanup"));
    assert_eq!(hooks.keyboard, Some(13));
    assert!(
        hooks
            .install(
                false,
                |_| panic!("must not install before cleanup"),
                |_| Ok(())
            )
            .is_err()
    );
    hooks
        .release(|handle| {
            assert_eq!(handle, 13);
            Ok(())
        })
        .unwrap();
    assert!(hooks.keyboard.is_none());
}

#[test]
fn release_attempts_both_hooks_and_retries_only_the_failed_handle() {
    let mut hooks = Hooks::<u8>::new();
    hooks
        .install(
            false,
            |keyboard| Ok(if keyboard { 13 } else { 14 }),
            |_| Ok(()),
        )
        .unwrap();
    let mut released = Vec::new();
    assert!(
        hooks
            .release(|handle| {
                released.push(handle);
                if handle == 13 {
                    anyhow::bail!("busy")
                }
                Ok(())
            })
            .is_err()
    );
    assert_eq!(released, [13, 14]);
    assert_eq!(hooks.keyboard, Some(13));
    assert_eq!(hooks.mouse, None);
    hooks
        .release(|handle| {
            released.push(handle);
            Ok(())
        })
        .unwrap();
    assert_eq!(released, [13, 14, 13]);
}

#[test]
fn source_debugger_branch_installs_keyboard_only() {
    let mut hooks = Hooks::<u8>::new();
    hooks
        .install(
            true,
            |keyboard| {
                assert!(keyboard);
                Ok(13)
            },
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(hooks.keyboard, Some(13));
    assert_eq!(hooks.mouse, None);
    hooks.release(|_| Ok(())).unwrap();
}
