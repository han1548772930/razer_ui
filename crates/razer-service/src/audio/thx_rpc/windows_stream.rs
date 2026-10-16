//! Nonblocking Winsock AF_UNIX adapter in a worker-owned Tokio runtime.
//! The adapter is deliberately local-only: it accepts a filesystem socket path
//! and never resolves a host name or opens TCP.
use super::socket_lifecycle::{OwnedSocket, SocketControl};
use anyhow::{Result, ensure};
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use windows_sys::Win32::Networking::WinSock as ws;

pub struct LocalUnixStream {
    socket: OwnedSocket<WinsockSocket>,
    read_wake: Option<Pin<Box<tokio::time::Sleep>>>,
    write_wake: Option<Pin<Box<tokio::time::Sleep>>>,
}
struct WinsockSocket {
    socket: ws::SOCKET,
    _winsock: Winsock,
}
impl SocketControl for WinsockSocket {
    fn shutdown_send(&mut self) -> io::Result<()> {
        if unsafe { ws::shutdown(self.socket, ws::SD_SEND) } == 0 {
            Ok(())
        } else {
            Err(last_io_error())
        }
    }
    fn shutdown_both(&mut self) {
        unsafe {
            let _ = ws::shutdown(self.socket, ws::SD_BOTH);
        }
    }
    fn close(&mut self) {
        unsafe {
            let _ = ws::closesocket(self.socket);
        }
    }
}
struct Winsock;
impl Drop for Winsock {
    fn drop(&mut self) {
        unsafe {
            let _ = ws::WSACleanup();
        }
    }
}

impl LocalUnixStream {
    pub async fn connect(path: &str) -> Result<Self> {
        ensure!(
            !path.is_empty() && !path.contains('\0'),
            "THX local socket path is empty or contains NUL"
        );
        let bytes = path.as_bytes();
        ensure!(
            bytes.len() + 1 <= 108,
            "THX AF_UNIX path exceeds Windows limit"
        );
        // One balanced Winsock reference per stream; every early return also
        // releases it. No socket/API operation waits for network readiness.
        let startup = unsafe {
            let mut data = ws::WSADATA::default();
            ws::WSAStartup(0x0202, &mut data)
        };
        ensure!(startup == 0, "Winsock startup failed: {startup}");
        let winsock = Winsock;
        let socket = unsafe { ws::socket(ws::AF_UNIX as i32, ws::SOCK_STREAM, 0) };
        if socket == ws::INVALID_SOCKET {
            return Err(last_error("AF_UNIX socket"));
        }
        let result = Self {
            socket: OwnedSocket::new(WinsockSocket {
                socket,
                _winsock: winsock,
            }),
            read_wake: None,
            write_wake: None,
        };
        let mut nonblocking = 1;
        if unsafe { ws::ioctlsocket(socket, ws::FIONBIO, &mut nonblocking) } != 0 {
            return Err(last_error("AF_UNIX nonblocking setup"));
        }
        let mut address = ws::SOCKADDR_UN::default();
        address.sun_family = ws::AF_UNIX;
        for (slot, byte) in address.sun_path.iter_mut().zip(bytes.iter().copied()) {
            *slot = byte as i8;
        }
        let size = (2 + bytes.len() + 1) as i32;
        let status =
            unsafe { ws::connect(socket, (&address as *const ws::SOCKADDR_UN).cast(), size) };
        if status != 0 {
            let error = unsafe { ws::WSAGetLastError() };
            ensure!(
                matches!(
                    error,
                    ws::WSAEWOULDBLOCK | ws::WSAEINPROGRESS | ws::WSAEALREADY
                ),
                "AF_UNIX connect failed: {error}"
            );
            loop {
                let mut poll = ws::WSAPOLLFD {
                    fd: socket,
                    events: ws::POLLWRNORM,
                    revents: 0,
                };
                let ready = unsafe { ws::WSAPoll(&mut poll, 1, 0) };
                if ready < 0 {
                    return Err(last_error("AF_UNIX connect poll"));
                }
                if ready != 0 {
                    let mut error: i32 = 0;
                    let mut size = std::mem::size_of::<i32>() as i32;
                    if unsafe {
                        ws::getsockopt(
                            socket,
                            ws::SOL_SOCKET,
                            ws::SO_ERROR,
                            (&mut error as *mut i32).cast(),
                            &mut size,
                        )
                    } != 0
                    {
                        return Err(last_error("AF_UNIX connect SO_ERROR"));
                    }
                    ensure!(error == 0, "AF_UNIX connect failed: {error}");
                    ensure!(
                        poll.revents & ws::POLLWRNORM != 0,
                        "AF_UNIX connect ended without readiness"
                    );
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        Ok(result)
    }
    fn recv(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = unsafe {
            ws::recv(
                self.socket.inner().socket,
                bytes.as_mut_ptr(),
                bytes.len().try_into().unwrap_or(i32::MAX),
                0,
            )
        };
        if count >= 0 {
            Ok(count as usize)
        } else {
            Err(last_io_error())
        }
    }
    fn send(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.socket.write(|socket| {
            let count = unsafe {
                ws::send(
                    socket.socket,
                    bytes.as_ptr(),
                    bytes.len().try_into().unwrap_or(i32::MAX),
                    0,
                )
            };
            if count >= 0 {
                Ok(count as usize)
            } else {
                Err(last_io_error())
            }
        })
    }
}
impl AsyncRead for LocalUnixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if out.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let spare = out.initialize_unfilled();
        match self.recv(spare) {
            Ok(0) => Poll::Ready(Ok(())),
            Ok(count) => {
                self.read_wake = None;
                out.advance(count);
                Poll::Ready(Ok(()))
            }
            Err(error) if error.raw_os_error() == Some(ws::WSAEWOULDBLOCK) => {
                arm(&mut self.read_wake, cx)
            }
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}
impl AsyncWrite for LocalUnixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.send(bytes) {
            Ok(count) => {
                self.write_wake = None;
                Poll::Ready(Ok(count))
            }
            Err(error) if error.raw_os_error() == Some(ws::WSAEWOULDBLOCK) => {
                arm(&mut self.write_wake, cx)
            }
            Err(error) => Poll::Ready(Err(error)),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.socket.shutdown_send())
    }
}
// Winsock AF_UNIX readiness is checked by nonblocking recv/send. An owned Tokio
// timer supplies the wakeup; independent read/write timers avoid waker loss.
fn arm<T>(
    slot: &mut Option<Pin<Box<tokio::time::Sleep>>>,
    cx: &mut Context<'_>,
) -> Poll<io::Result<T>> {
    let timer = slot.get_or_insert_with(|| Box::pin(tokio::time::sleep(Duration::from_millis(5))));
    if timer.as_mut().poll(cx).is_ready() {
        *slot = None;
        let mut timer = Box::pin(tokio::time::sleep(Duration::from_millis(5)));
        let _ = timer.as_mut().poll(cx);
        *slot = Some(timer);
    }
    Poll::Pending
}
fn last_error(context: &str) -> anyhow::Error {
    anyhow::anyhow!("{context} failed: {}", last_io_error())
}
fn last_io_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { ws::WSAGetLastError() })
}
