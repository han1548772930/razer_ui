//! Socket ownership and write-half state, independent of the OS calls.
use std::io;

pub(super) trait SocketControl {
    fn shutdown_send(&mut self) -> io::Result<()>;
    fn shutdown_both(&mut self);
    fn close(&mut self);
}

pub(super) struct OwnedSocket<S: SocketControl> {
    inner: S,
    send_closed: bool,
}
impl<S: SocketControl> OwnedSocket<S> {
    pub(super) fn new(inner: S) -> Self {
        Self {
            inner,
            send_closed: false,
        }
    }
    pub(super) fn inner(&self) -> &S {
        &self.inner
    }
    pub(super) fn write(
        &mut self,
        operation: impl FnOnce(&mut S) -> io::Result<usize>,
    ) -> io::Result<usize> {
        if self.send_closed {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        operation(&mut self.inner)
    }
    pub(super) fn shutdown_send(&mut self) -> io::Result<()> {
        if !self.send_closed {
            // A failed OS call has not established a closed sending half.
            // Keep the handle owned and permit an explicit subsequent retry.
            self.inner.shutdown_send()?;
            self.send_closed = true;
        }
        Ok(())
    }
}
impl<S: SocketControl> Drop for OwnedSocket<S> {
    fn drop(&mut self) {
        // Closing a sending half does not release the owned socket. Every
        // exit still shuts down both halves and closes exactly once, before
        // the OS initialization/reference guard inside S is released.
        self.inner.shutdown_both();
        self.inner.close();
    }
}

#[cfg(test)]
mod thx_socket_lifecycle_tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Write(Vec<u8>),
        ShutdownSendFailed,
        ShutdownSend,
        ShutdownBoth,
        Close,
        ReleaseReference,
    }
    #[derive(Default)]
    struct State {
        events: Vec<Event>,
        shutdown_failures: usize,
        received: VecDeque<u8>,
    }
    struct FakeSocket(Rc<RefCell<State>>);
    impl FakeSocket {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .borrow_mut()
                .events
                .push(Event::Write(bytes.to_vec()));
            Ok(bytes.len())
        }
        fn read(&self) -> Vec<u8> {
            self.0.borrow_mut().received.drain(..).collect()
        }
    }
    impl SocketControl for FakeSocket {
        fn shutdown_send(&mut self) -> io::Result<()> {
            let mut state = self.0.borrow_mut();
            if state.shutdown_failures != 0 {
                state.shutdown_failures -= 1;
                state.events.push(Event::ShutdownSendFailed);
                Err(io::ErrorKind::Interrupted.into())
            } else {
                state.events.push(Event::ShutdownSend);
                Ok(())
            }
        }
        fn shutdown_both(&mut self) {
            self.0.borrow_mut().events.push(Event::ShutdownBoth);
        }
        fn close(&mut self) {
            self.0.borrow_mut().events.push(Event::Close);
        }
    }
    impl Drop for FakeSocket {
        fn drop(&mut self) {
            self.0.borrow_mut().events.push(Event::ReleaseReference);
        }
    }
    fn owner(state: &Rc<RefCell<State>>) -> OwnedSocket<FakeSocket> {
        OwnedSocket::new(FakeSocket(state.clone()))
    }

    #[test]
    fn failed_half_close_keeps_writes_and_retries_without_double_close() {
        let state = Rc::new(RefCell::new(State {
            shutdown_failures: 1,
            ..Default::default()
        }));
        {
            let mut socket = owner(&state);
            assert_eq!(socket.write(|socket| socket.write(b"before")).unwrap(), 6);
            assert_eq!(
                socket.shutdown_send().unwrap_err().kind(),
                io::ErrorKind::Interrupted
            );
            assert_eq!(
                socket
                    .write(|socket| socket.write(b"after failure"))
                    .unwrap(),
                13
            );
            socket.shutdown_send().unwrap();
            assert_eq!(
                socket
                    .write(|socket| socket.write(b"closed"))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::BrokenPipe
            );
            socket.shutdown_send().unwrap();
        }
        assert_eq!(
            state.borrow().events,
            [
                Event::Write(b"before".to_vec()),
                Event::ShutdownSendFailed,
                Event::Write(b"after failure".to_vec()),
                Event::ShutdownSend,
                Event::ShutdownBoth,
                Event::Close,
                Event::ReleaseReference,
            ]
        );
    }

    #[test]
    fn successful_half_close_preserves_received_bytes_until_owned_drop() {
        let state = Rc::new(RefCell::new(State {
            received: b"final response".iter().copied().collect(),
            ..Default::default()
        }));
        {
            let mut socket = owner(&state);
            socket.shutdown_send().unwrap();
            assert_eq!(socket.inner().read(), b"final response".as_slice());
            assert!(socket.write(|socket| socket.write(b"forbidden")).is_err());
            assert_eq!(state.borrow().events, [Event::ShutdownSend]);
        }
        assert_eq!(
            state.borrow().events,
            [
                Event::ShutdownSend,
                Event::ShutdownBoth,
                Event::Close,
                Event::ReleaseReference,
            ]
        );
    }

    #[test]
    fn setup_error_and_failed_half_close_release_handle_before_reference() {
        fn fail_after_owning(state: &Rc<RefCell<State>>, half_close: bool) -> io::Result<()> {
            let mut socket = owner(state);
            if half_close {
                socket.shutdown_send()?;
            }
            Err(io::ErrorKind::Other.into())
        }
        for half_close in [false, true] {
            let state = Rc::new(RefCell::new(State {
                shutdown_failures: 1,
                ..Default::default()
            }));
            assert!(fail_after_owning(&state, half_close).is_err());
            let expected = if half_close {
                vec![
                    Event::ShutdownSendFailed,
                    Event::ShutdownBoth,
                    Event::Close,
                    Event::ReleaseReference,
                ]
            } else {
                vec![Event::ShutdownBoth, Event::Close, Event::ReleaseReference]
            };
            assert_eq!(state.borrow().events, expected);
        }
    }
}
