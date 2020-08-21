use crate::simpletcp::{TcpServer, TcpStream};
use std::io::Write;
use std::net;
use std::thread::spawn;

#[test]
fn create_server() {
    TcpServer::new("127.0.0.1:1234").expect("Failed to create server");
}

#[test]
fn accept_none() {
    let server = TcpServer::new("127.0.0.1:1235").expect("Failed to create server");
    match server.accept().expect("Accept failed") {
        None => {}
        Some(_) => {
            panic!("Accept returned Some but None was expected");
        }
    }
}

#[test]
fn connect() {
    let _server = TcpServer::new("127.0.0.1:1236").expect("Failed to create server");
    let _client = TcpStream::connect("127.0.0.1:1236").expect("Failed to connect to server");
}

#[test]
fn accept() {
    let server = TcpServer::new("127.0.0.1:1237").expect("Failed to create server");
    let _client = TcpStream::connect("127.0.0.1:1237").expect("Failed to connect to server");

    match server.accept().expect("Failed to accept") {
        None => {
            panic!("Accept returned None but Some was expected");
        }
        Some(_) => {}
    }
}

#[test]
fn raw() {
    let server = TcpServer::new("127.0.0.1:1238").expect("Failed to create server");
    let mut client = TcpStream::connect("127.0.0.1:1238").expect("Failed to connect to server");

    let mut s_client = server.accept().unwrap().unwrap();

    client.write_raw(&[1, 2, 3]).unwrap();
    let recv = s_client.read_raw().unwrap();

    assert_eq!(recv, Some(vec![1, 2, 3]));
}

#[test]
fn raw_would_block() {
    let _server = TcpServer::new("127.0.0.1:1239").expect("Failed to create server");
    let mut client = TcpStream::connect("127.0.0.1:1239").expect("Failed to connect to server");

    assert_eq!(client.read_raw().unwrap(), None);
}

#[test]
fn raw_fragmented() {
    let server = TcpServer::new("127.0.0.1:1240").expect("Failed to create server");
    let mut client =
        net::TcpStream::connect("127.0.0.1:1240").expect("Failed to connect to server");
    let mut s_client = server.accept().unwrap().unwrap();

    client.write(&[3, 0]).unwrap();
    assert_eq!(s_client.read_raw().unwrap(), None);

    client.write(&[0]).unwrap();
    assert_eq!(s_client.read_raw().unwrap(), None);

    client.write(&[0, 1, 2]).unwrap();
    assert_eq!(s_client.read_raw().unwrap(), None);

    client.write(&[3, 1, 0]).unwrap();
    assert_eq!(s_client.read_raw().unwrap(), Some(vec![1, 2, 3]));

    client.write(&[0, 0, 7]).unwrap();
    assert_eq!(s_client.read_raw().unwrap(), Some(vec![7]));
}

#[test]
fn client_write_server_read() {
    let server = TcpServer::new("127.0.0.1:1241").expect("Failed to create server");
    spawn(|| {
        let mut client = TcpStream::connect("127.0.0.1:1241").expect("Failed to connect to server");
        client.wait_until_ready().unwrap();
        client.write(&[1, 2, 3]).unwrap();
    });

    loop {
        let s_client = server.accept().unwrap();
        match s_client {
            None => {}
            Some(mut s_client) => {
                s_client.wait_until_ready().unwrap();
                assert_eq!(s_client.read().unwrap(), Some(vec![1, 2, 3]));
                break;
            }
        }
    }
}

#[test]
fn server_write_client_read() {
    let server = TcpServer::new("127.0.0.1:1242").expect("Failed to create server");

    spawn(move || loop {
        let s_client = server.accept().unwrap();
        match s_client {
            None => {}
            Some(mut s_client) => {
                s_client.wait_until_ready().unwrap();
                s_client.write(&[1, 2, 3]).unwrap();
                break;
            }
        }
    });

    let mut client = TcpStream::connect("127.0.0.1:1242").expect("Failed to connect to server");
    client.wait_until_ready().unwrap();

    loop {
        match client.read().unwrap() {
            None => {}
            Some(msg) => {
                assert_eq!(msg, vec![1, 2, 3]);
                break;
            }
        }
    }
}
