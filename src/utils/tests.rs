use crate::utils;
use std::net::{TcpStream, TcpListener};
use std::thread::{spawn, sleep};
use std::io::Write;
use std::time::{Duration, Instant};
use crate::utils::EV_POLLIN;

#[test]
fn poll_read(){
    let server = TcpListener::bind("127.0.0.1:42345").unwrap();

    spawn(move || {
        let (mut socket, _) = server.accept().unwrap();
        sleep(Duration::from_secs(1));
        socket.write(&[1]).unwrap();
    });

    let test = TcpStream::connect("127.0.0.1:42345").unwrap();

    let time = Instant::now();
    let success = utils::poll(&test, EV_POLLIN);
    let time = time.elapsed().as_millis();

    assert!(success);
    assert!(time >= 990 && time < 1100);
}

#[test]
fn poll_read_timeout(){
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
    assert!(time >= 490 && time < 550);
}