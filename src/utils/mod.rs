#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(windows)]
use std::os::windows::io::AsRawSocket;

mod platform {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[cfg(test)]
mod tests;

pub const EV_POLLIN: i16 = 1 << 0;
pub const EV_POLLOUT: i16 = 1 << 1;

/// Polls the socket
///
/// # Arguments
///
/// * `socket` - Socket to poll
/// * `event` - Event to poll ([EV_POLLIN](constant.EV_POLLIN.html) or [EV_POLLOUT](constant.EV_POLLOUT.html))
///
/// # Returns
/// `true` if polled event has occured, `false` if not
#[cfg(unix)]
pub fn poll<A: AsRawFd>(socket: &A, event: i16) -> bool {
    let fd = socket.as_raw_fd();
    unsafe {
        let translated_events = translate_event(event);
        let revents = platform::c_poll(fd, translated_events, -1);
        return revents == translated_events;
    }
}

/// Polls the socket with timeout
///
/// # Arguments
///
/// * `socket` - Socket to poll
/// * `event` - Event to poll ([EV_POLLIN](constant.EV_POLLIN.html) or [EV_POLLOUT](constant.EV_POLLOUT.html))
/// * `timeout` - Timeout in milliseconds
///
/// # Returns
/// `true` if polled event has occured, `false` if not
#[cfg(unix)]
pub fn poll_timeout<A: AsRawFd>(socket: &A, event: i16, timeout: i32) -> bool {
    let fd = socket.as_raw_fd();
    unsafe {
        let translated_events = translate_event(event);
        let revents = platform::c_poll(fd, translated_events, timeout);
        return revents == translated_events;
    }
}

/// Polls the socket
///
/// # Arguments
///
/// * `socket` - Socket to poll
/// * `event` - Event to poll ([EV_POLLIN](constant.EV_POLLIN.html) or [EV_POLLOUT](constant.EV_POLLOUT.html))
///
/// # Returns
/// `true` if polled event has occured, `false` if not
#[cfg(windows)]
pub fn poll<A: AsRawSocket>(socket: &A, event: i16) -> bool {
    let fd = socket.as_raw_socket();
    unsafe {
        let translated_events = translate_event(event);
        let revents = platform::c_poll(fd, translated_events, -1);
        return revents == translated_events;
    }
}

/// Polls the socket with timeout
///
/// # Arguments
///
/// * `socket` - Socket to poll
/// * `event` - Event to poll ([EV_POLLIN](constant.EV_POLLIN.html) or [EV_POLLOUT](constant.EV_POLLOUT.html))
/// * `timeout` - Timeout in milliseconds
///
/// # Returns
/// `true` if polled event has occured, `false` if not
#[cfg(windows)]
pub fn poll_timeout<A: AsRawSocket>(socket: &A, event: i16, timeout: i32) -> bool {
    let fd = socket.as_raw_socket();
    unsafe {
        let translated_events = translate_event(event);
        let revents = platform::c_poll(fd, translated_events, timeout);
        return revents == translated_events;
    }
}

unsafe fn translate_event(ev: i16) -> i16 {
    let mut translated = 0;
    if (ev & EV_POLLIN) != 0 {
        translated |= platform::ev_pollin;
    }
    if (ev & EV_POLLOUT) != 0 {
        translated |= platform::ev_pollout;
    }
    translated
}
