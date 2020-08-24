#[cfg(unix)]
use std::os::unix::io::AsRawFd;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests;

pub const EV_POLLIN: i16 = 1 << 0;
pub const EV_POLLOUT: i16 = 1 << 1;

#[cfg(unix)]
pub fn poll<A: AsRawFd>(socket: &A, event: i16) -> bool{
    let fd = socket.as_raw_fd();
    unsafe {
        let translated_events = translate_event(event);
        let revents = c_poll(fd, translated_events, -1);
        return revents == translated_events;
    }
}

#[cfg(unix)]
pub fn poll_timeout<A: AsRawFd>(socket: &A, event: i16, timeout: i32) -> bool{
    let fd = socket.as_raw_fd();
    unsafe {
        let translated_events = translate_event(event);
        let revents = c_poll(fd, translated_events, timeout);
        return revents == translated_events;
    }
}

#[cfg(windows)]
pub fn poll<A: AsRawFd>(socket: &A, event: i16) -> bool{
    unimplemented!("Windows is not supported.");
}

#[cfg(windows)]
pub fn poll_timeout<A: AsRawFd>(socket: &A, event: i16, timeout: i32) -> bool{
    unimplemented!("Windows is not supported.");
}

unsafe fn translate_event(ev: i16) -> i16{
    let mut translated = 0;
    if (ev & EV_POLLIN) != 0 { translated |= ev_pollin; }
    if (ev & EV_POLLOUT) != 0 { translated |= ev_pollout; }
    translated
}