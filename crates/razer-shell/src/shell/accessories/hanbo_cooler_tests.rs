use super::restoration_session;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Session {
    owner: u64,
    accepting: bool,
}

fn accepting_same_owner(initial: &Session, current: &Session) -> bool {
    initial.owner == current.owner && current.accepting
}

#[::core::prelude::v1::test]
fn expired_startup_completion_never_creates_a_successor() {
    let initial = Session {
        owner: 1,
        accepting: true,
    };
    for current in [
        None,
        Some(Session {
            accepting: false,
            ..initial
        }),
        Some(Session {
            owner: 2,
            accepting: true,
        }),
    ] {
        let creations = Cell::new(0);
        let result = restoration_session(Some(&initial), current, accepting_same_owner, || {
            creations.set(creations.get() + 1);
            Ok(Session {
                owner: 3,
                accepting: true,
            })
        });
        assert!(result.is_err());
        assert_eq!(creations.get(), 0);
    }
    assert_eq!(
        restoration_session(
            Some(&initial),
            Some(initial),
            accepting_same_owner,
            || panic!("A live initial restore must not create another session"),
        )
        .unwrap(),
        initial,
    );
}

#[::core::prelude::v1::test]
fn ordinary_profile_save_can_request_a_current_session() {
    let creations = Cell::new(0);
    let requested = Session {
        owner: 2,
        accepting: true,
    };
    let session = restoration_session(
        None,
        Some(Session {
            owner: 1,
            accepting: false,
        }),
        accepting_same_owner,
        || {
            creations.set(creations.get() + 1);
            Ok(requested)
        },
    )
    .unwrap();
    assert_eq!(creations.get(), 1);
    assert_eq!(session, requested);
    assert_eq!(
        restoration_session::<Session>(None, None, accepting_same_owner, || Err(
            "session creation failed".into()
        )),
        Err("session creation failed".into()),
    );
}
