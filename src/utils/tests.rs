use crate::utils;
use crate::utils::{EV_POLLIN, EV_POLLOUT};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};

#[test]
fn poll_read() {
    let server = TcpListener::bind("127.0.0.1:42345").unwrap();

    spawn(move || {
        let (mut socket, _) = server.accept().unwrap();
        sleep(Duration::from_secs(1));
        socket.write(&[1]).unwrap();
        sleep(Duration::from_millis(500));
    });

    let test = TcpStream::connect("127.0.0.1:42345").unwrap();

    let time = Instant::now();
    let success = utils::poll(&test, EV_POLLIN);
    let time = time.elapsed().as_millis();

    assert!(success);
    assert!(time >= 800 && time < 1100);
}

#[test]
fn poll_both() {
    let server = TcpListener::bind("127.0.0.1:12345").unwrap();

    spawn(move || {
        let (mut socket, _) = server.accept().unwrap();
        sleep(Duration::from_secs(1));
        socket.write(&[1]).unwrap();
        sleep(Duration::from_millis(500));
    });

    let test = TcpStream::connect("127.0.0.1:12345").unwrap();

    let time = Instant::now();
    let success = utils::poll(&test, EV_POLLIN | EV_POLLOUT);
    let time = time.elapsed().as_millis();

    assert!(success);
    assert!(time < 10);
}

#[test]
fn poll_read_timeout() {
    let server = TcpListener::bind("127.0.0.1:42365").unwrap();

    spawn(move || {
        let (mut socket, _) = server.accept().unwrap();
        sleep(Duration::from_secs(1));
        socket.write(&[1]).unwrap();
    });

    let test = TcpStream::connect("127.0.0.1:42365").unwrap();

    let time = Instant::now();
    let success = utils::poll_timeout(&test, EV_POLLIN, 500);
    let time = time.elapsed().as_millis();

    assert!(!success);
    assert!(time >= 450 && time < 550);
}

#[test]
fn poll_set() {
    let server = TcpListener::bind("127.0.0.1:43365").unwrap();

    spawn(|| {
        let mut sockets = Vec::new();
        loop {
            sockets.push(TcpStream::connect("127.0.0.1:43365").unwrap());
            if sockets.len() == 20 {
                break;
            }
        }
        sockets[4].write_all(&[1]).unwrap();
        sleep(Duration::from_millis(100));
    });
    let mut sockets = Vec::new();

    loop {
        let (socket, _) = server.accept().unwrap();
        sockets.push(socket);
        if sockets.len() == 20 {
            break;
        }
    }

    let mut fds = utils::get_fd_array(&sockets);
    let res = utils::poll_set(&mut fds, EV_POLLIN);
    assert_eq!(res, 4);
}

#[test]
fn poll_set_timeout() {
    let server = TcpListener::bind("127.0.0.1:43365").unwrap();

    spawn(|| {
        let mut sockets = Vec::new();
        loop {
            sockets.push(TcpStream::connect("127.0.0.1:43365").unwrap());
            if sockets.len() == 20 {
                break;
            }
        }

        sleep(Duration::from_millis(1100));
    });
    let mut sockets = Vec::new();

    loop {
        let (socket, _) = server.accept().unwrap();
        sockets.push(socket);
        if sockets.len() == 20 {
            break;
        }
    }

    let mut fds = utils::get_fd_array(&sockets);
    let time = Instant::now();
    let res = utils::poll_set_timeout(&mut fds, EV_POLLIN, 1000);
    let time = time.elapsed().as_millis();
    assert_eq!(res, None);
    assert!(time > 800 && time < 1100);
}
