use std::convert::TryInto;
use std::fmt;
use std::fmt::Formatter;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net;
use std::net::ToSocketAddrs;

#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};

#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, RawSocket};

extern crate openssl;

use openssl::error::ErrorStack;
use openssl::pkey::Private;
use openssl::rsa::Padding;
use openssl::rsa::Rsa;
use openssl::symm;
use openssl::symm::Cipher;

extern crate rand;

use rand::prelude::StdRng;
use rand::RngCore;
use rand::SeedableRng;

use crate::utils::{poll, poll_timeout, EV_POLLIN, EV_POLLOUT};
use std::time::Instant;
use MessageError::UnexpectedEnd;
use State::{NotInitialized, Ready, WaitingForPublicKey, WaitingForSymmKey};
use crate::simpletcp::Error::TcpError;
use std::collections::VecDeque;

#[cfg(test)]
mod tests;

/// Error returned by all functions of TcpStream and TcpServer
pub enum Error {
    /// TcpStream is not ready yet
    ///
    /// Call [wait_until_ready](struct.TcpStream.html#method.wait_until_ready) or wait until [get_ready](struct.TcpStream.html#method.get_ready) returns `true`
    NotReady,

    /// An error occurred during encryption/decryption
    EncryptionError(ErrorStack),

    /// An error occurred during TCP operation
    TcpError(io::Error),

    /// TCP connection was closed
    ConnectionClosed,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            Error::NotReady => f.write_str("Error::NotReady"),
            Error::EncryptionError(openssl_err) => {
                f.write_fmt(format_args!("Error::EncryptionError: {}", openssl_err))
            }
            Error::TcpError(io_err) => f.write_fmt(format_args!("Error::TcpError: {}", io_err)),
            Error::ConnectionClosed => f.write_str("Error::ConnectionClosed"),
        };
    }
}

impl From<io::Error> for Error {
    fn from(io_err: io::Error) -> Self {
        Error::TcpError(io_err)
    }
}

impl From<ErrorStack> for Error {
    fn from(openssl_err: ErrorStack) -> Self {
        Error::EncryptionError(openssl_err)
    }
}

/// Internal state of [TcpStream](struct.TcpStream.html)
#[derive(PartialEq)]
pub enum State {
    /// [TcpStream](struct.TcpStream.html) is not initialized
    NotInitialized,

    /// [TcpStream](struct.TcpStream.html) is waiting for server to send public key so it can encrypt symmetric key
    WaitingForPublicKey,

    /// [TcpStream](struct.TcpStream.html) sent public key to the client and is waiting for client to send symmetric key
    WaitingForSymmKey,

    /// Key was negotiated and [TcpStream](struct.TcpStream.html) is ready to send and receive data
    Ready,
}

/// TCP Server
///
/// TcpServer used to accept new [TcpStreams](struct.TcpStream.html)
pub struct TcpServer {
    socket: net::TcpListener,
    key: Rsa<Private>,
}

impl TcpServer {
    /// Creates new TcpServer
    ///
    /// # Arguments
    ///
    /// * `addr` - Address to listen on
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
        let key = Rsa::generate(4096)?;
        Self::new_with_key(addr, key)
    }

    fn new_with_key<A: ToSocketAddrs>(addr: A, key: Rsa<Private>) -> Result<Self, Error> {
        let socket = net::TcpListener::bind(addr)?;
        socket.set_nonblocking(true)?;
        return Ok(Self { socket, key });
    }

    /// Accepts a client
    ///
    /// # Returns
    ///
    /// Returns accepted [TcpStream](struct.TcpStream.html) or `None` if there is no new connection
    pub fn accept(&self) -> Result<Option<TcpStream>, Error> {
        match self.socket.accept() {
            Ok((socket, _addr)) => {
                let mut stream = TcpStream::from_socket(socket)?;
                stream.server_init(&self.key)?;
                Ok(Some(stream))
            }
            Err(io_err) => match io_err.kind() {
                ErrorKind::WouldBlock => Ok(None),
                _ => Err(Error::TcpError(io_err)),
            },
        }
    }

    /// Accepts a client blocking
    ///
    /// # Returns
    ///
    /// Returns accepted [TcpStream](struct.TcpStream.html)
    pub fn accept_blocking(&self) -> Result<TcpStream, Error> {
        loop {
            match self.accept()? {
                None => {
                    poll(self, EV_POLLIN);
                }
                Some(client) => {
                    return Ok(client);
                }
            };
        }
    }
}

#[cfg(unix)]
impl AsRawFd for TcpServer {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for TcpServer {
    fn as_raw_socket(&self) -> RawSocket {
        self.socket.as_raw_socket()
    }
}

macro_rules! try_io {
    ($r: expr, $wb_closure: expr) => {
        match $r {
            Ok(res) => res,
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => {
                    $wb_closure();
                    return Ok(None);
                }
                _ => {
                    return Err(Error::TcpError(e));
                }
            },
        }
    };
}

/// Encrypted TCP stream
///
/// Communication is encrypted using 256-bit AES-CBC, key is negotiated using 4096-bit RSA.
pub struct TcpStream {
    socket: net::TcpStream,
    read_buffer: Vec<u8>,
    write_buffer: DequeueBuffer,
    key: [u8; 32],
    state: State,
    rsa_key: Option<Rsa<Private>>,
    rand: StdRng,
}

impl TcpStream {
    fn from_socket(socket: net::TcpStream) -> Result<Self, Error> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            read_buffer: Vec::new(),
            write_buffer: DequeueBuffer::new(),
            key: Default::default(),
            state: NotInitialized,
            rsa_key: None,
            rand: StdRng::from_entropy(),
        })
    }

    /// Connects to remote [TcpServer](struct.TcpServer.html)
    ///
    /// # Arguments
    ///
    /// * `addr` - Address of remote [TcpServer](struct.TcpServer.html)
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
        let socket = net::TcpStream::connect(addr)?;
        socket.set_nonblocking(true)?;

        Ok(Self {
            socket,
            read_buffer: Vec::new(),
            write_buffer: DequeueBuffer::new(),
            key: Default::default(),
            state: WaitingForPublicKey,
            rsa_key: None,
            rand: StdRng::from_entropy(),
        })
    }

    fn server_init(&mut self, rsa_key: &Rsa<Private>) -> Result<(), Error> {
        self.write_raw(&rsa_key.public_key_to_der()?)?;
        self.rsa_key = Some(rsa_key.clone());
        self.state = WaitingForSymmKey;
        Ok(())
    }

    fn init_step(&mut self) -> Result<(), Error> {
        match self.state {
            NotInitialized => panic!("TcpStream init_step state NotInitialized"),
            WaitingForPublicKey => match self.read_raw()? {
                Some(rsa_key) => {
                    self.rand.fill_bytes(&mut self.key);

                    let rsa_key = Rsa::public_key_from_der(&rsa_key)?;
                    let mut encrypted_key: Vec<u8> = vec![0; rsa_key.size() as usize];
                    let encrypted_size = rsa_key.public_encrypt(
                        &self.key,
                        &mut encrypted_key,
                        Padding::PKCS1_OAEP,
                    )?;
                    encrypted_key.resize(encrypted_size, 0);
                    self.write_raw(&encrypted_key)?;
                    self.state = Ready;
                }
                None => {}
            },
            WaitingForSymmKey => match self.read_raw()? {
                Some(encrypted_key) => {
                    let rsa_key = self.rsa_key.as_ref().unwrap();
                    let mut key: Vec<u8> = vec![0; rsa_key.size() as usize];
                    let key_size =
                        rsa_key.private_decrypt(&encrypted_key, &mut key, Padding::PKCS1_OAEP)?;
                    key.resize(key_size, 0);
                    assert_eq!(key_size, 32);

                    self.key.copy_from_slice(&key);
                    self.state = Ready;
                }
                None => {}
            },
            _ => {}
        }

        Ok(())
    }

    /// Reads a message non-blocking
    ///
    /// # Returns
    /// Returns `Some(Message)` or `None` if no message has arrived
    pub fn read(&mut self) -> Result<Option<Message>, Error> {
        if self.state != Ready {
            return Err(Error::NotReady);
        }

        match self.read_raw()? {
            None => Ok(None),
            Some(buf) => {
                let iv = &buf[..16];
                let decrypted =
                    symm::decrypt(Cipher::aes_256_cbc(), &self.key, Some(iv), &buf[16..])?;

                Ok(Some(Message::from_buffer(decrypted)))
            }
        }
    }

    /// Reads a message blocking
    ///
    /// # Returns
    /// Returns [Message](struct.Message.html)
    pub fn read_blocking(&mut self) -> Result<Message, Error> {
        loop {
            match self.read()? {
                None => {
                    poll(self, EV_POLLIN);
                }
                Some(msg) => {
                    return Ok(msg);
                }
            }
        }
    }

    /// Reads a message blocking with timeout
    ///
    /// # Arguments
    ///
    /// * `timeout` - Timeout in milliseconds
    /// # Returns
    /// Returns `Some(Message)` or `None` if reading timed out
    pub fn read_timeout(&mut self, timeout: i32) -> Result<Option<Message>, Error> {
        let time = Instant::now();
        loop {
            match self.read()? {
                None => {
                    if timeout < time.elapsed().as_millis() as i32 {
                        return Ok(None);
                    }
                    if !poll_timeout(self, EV_POLLIN, timeout) {
                        return Ok(None);
                    }
                }
                Some(msg) => {
                    return Ok(Some(msg));
                }
            }
        }
    }

    /// Writes a message
    ///
    /// Message may not be written completely, you should call [flush](struct.TcpStream.html#method.flush) afterwards
    /// # Arguments
    ///
    /// * `msg` - Message to be sent
    pub fn write(&mut self, msg: &Message) -> Result<(), Error> {
        if self.state != Ready {
            return Err(Error::NotReady);
        }

        let mut iv = [0; 16];
        self.rand.fill_bytes(&mut iv);

        let mut encrypted =
            symm::encrypt(Cipher::aes_256_cbc(), &self.key, Some(&iv), &msg.buffer)?;

        let mut raw = iv.to_vec();
        raw.append(&mut encrypted);
        self.write_raw(&raw)
    }

    /// Writes a message and blocks until it's completely flushed
    ///
    /// # Arguments
    ///
    /// * `msg` - Message to be sent
    pub fn write_blocking(&mut self, msg: &Message) -> Result<(), Error> {
        if self.state != Ready {
            return Err(Error::NotReady);
        }

        let mut iv = [0; 16];
        self.rand.fill_bytes(&mut iv);

        let mut encrypted =
            symm::encrypt(Cipher::aes_256_cbc(), &self.key, Some(&iv), &msg.buffer)?;

        let mut raw = iv.to_vec();
        raw.append(&mut encrypted);
        self.write_raw(&raw)?;

        while !self.flush().unwrap() {
            poll(self, EV_POLLOUT);
        }

        Ok(())
    }

    /// Blocks the thread until connection is ready to read and write messages
    pub fn wait_until_ready(&mut self) -> Result<(), Error> {
        while !self.get_ready()? {
            poll(self, EV_POLLIN);
        }

        Ok(())
    }

    /// Tries to complete connection initialization
    ///
    /// # Returns
    /// Returns `true` if connection is ready, `false` otherwise
    pub fn get_ready(&mut self) -> Result<bool, Error> {
        if self.state == Ready {
            return Ok(true);
        }
        match self.init_step() {
            Err(e) => return match e {
                Error::TcpError(io_err) if io_err.kind() == ErrorKind::WouldBlock => {
                    Ok(false)
                }
                _ => {
                    Err(e)
                }
            },
            _ => {}
        }

        Ok(self.state == Ready)
    }

    /// Attempts to flush pending write operations
    ///
    /// # Returns
    /// `true` if all pending operations were flushed, `false` if there are more operations to flush
    pub fn flush(&mut self) -> Result<bool, Error>{
        while poll_timeout(self, EV_POLLOUT, 0) {
            if self.write_buffer.is_empty() {
                return Ok(true);
            }
            let bytes_written = self.socket.write(self.write_buffer.peek());
            if bytes_written.is_err() {
                let err = bytes_written.as_ref().err().unwrap();
                if err.kind() != ErrorKind::WouldBlock {
                    return Err(TcpError(bytes_written.err().unwrap()));
                }
            } else if bytes_written.as_ref().unwrap() == &0 {
                return Err(Error::ConnectionClosed);
            }

            let bytes_written = bytes_written.unwrap_or(0);
            self.write_buffer.advance(bytes_written);
        }
        Ok(self.write_buffer.is_empty())
    }

    fn write_raw(&mut self, msg: &[u8]) -> Result<(), Error> {
        let length = msg.len() as u32;
        let length_bytes = length.to_le_bytes();
        let bytes_written = self.socket.write(&length_bytes);
        if bytes_written.is_err() {
            let err = bytes_written.as_ref().err().unwrap();
            if err.kind() != ErrorKind::WouldBlock {
                return Err(TcpError(bytes_written.err().unwrap()));
            }
        } else if bytes_written.as_ref().unwrap() == &0 {
            return  Err(Error::ConnectionClosed);
        }

        let bytes_written = bytes_written.unwrap_or(0);
        if bytes_written != 4 {
            self.write_buffer.enqueue(&length_bytes[bytes_written..]);
        }
        let bytes_written = self.socket.write(msg);

        if bytes_written.is_err() {
            let err = bytes_written.as_ref().err().unwrap();
            if err.kind() != ErrorKind::WouldBlock {
                return Err(TcpError(bytes_written.err().unwrap()));
            }
        } else if bytes_written.as_ref().unwrap() == &0 {
            return  Err(Error::ConnectionClosed);
        }

        let bytes_written = bytes_written.unwrap_or(0);
        if bytes_written != msg.len() {
           self.write_buffer.enqueue(&msg[bytes_written..]);
        }

        Ok(())
    }

    fn read_raw(&mut self) -> Result<Option<Vec<u8>>, Error> {
        if self.read_buffer.len() < 4 {
            let start = self.read_buffer.len();
            self.read_buffer.resize(4, 0);
            let bytes_read = try_io!(self.socket.read(&mut self.read_buffer[start..]), || {
                self.read_buffer.resize(start, 0);
            });

            if bytes_read == 0 {
                return Err(Error::ConnectionClosed);
            }

            self.read_buffer.resize(start + bytes_read, 0);
            if self.read_buffer.len() != 4 {
                return Ok(None);
            }
        }

        let len = u32::from_le_bytes(self.read_buffer[..4].try_into().unwrap()) as usize;

        let start = self.read_buffer.len();
        self.read_buffer.resize(4 + len, 0);
        let bytes_read = try_io!(self.socket.read(&mut self.read_buffer[start..]), || {
            self.read_buffer.resize(start, 0);
        });

        if bytes_read == 0 {
            return Err(Error::ConnectionClosed);
        }

        self.read_buffer.resize(start + bytes_read, 0);
        if self.read_buffer.len() == len + 4 {
            let result = self.read_buffer[4..].to_vec();
            self.read_buffer.clear();
            return Ok(Some(result));
        }

        Ok(None)
    }

    pub fn set_nodelay(&mut self, val: bool) -> Result<(), Error>{
        self.socket.set_nodelay(val)?;
        Ok(())
    }

    pub fn nodelay(&self) -> Result<bool, Error>{
        let nodelay = self.socket.nodelay()?;
        Ok(nodelay)
    }
}

#[cfg(unix)]
impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for TcpStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.socket.as_raw_socket()
    }
}

struct DequeueBuffer{
    buffers: VecDeque<Vec<u8>>,
    start: usize
}

impl DequeueBuffer {
    fn new() -> Self{
        DequeueBuffer{
            buffers: VecDeque::new(),
            start: 0
        }
    }

    fn enqueue(&mut self, buf: &[u8]){
        self.buffers.push_back(buf.to_vec());
    }

    fn peek(&self) -> &[u8]{
        &self.buffers[0][self.start..]
    }

    fn advance(&mut self, n: usize){
        self.start += n;
        if self.start == self.buffers[0].len() {
            self.buffers.pop_front();
            self.start = 0;
        }
    }

    fn is_empty(&self) -> bool{
        self.buffers.is_empty()
    }
}

/// Message to be transmitted using [write](struct.TcpStream.html#method.write) or [read](struct.TcpStream.html#method.read)
pub struct Message {
    buffer: Vec<u8>,
    read_pos: usize,
}

/// Error occurred when encoding or decoding message
pub enum MessageError {
    UnexpectedEnd,
}

impl fmt::Debug for MessageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            UnexpectedEnd => {
                return f.write_str("Message has ended unexpectedly.");
            }
        }
    }
}

impl Message {
    /// Creates a new empty message
    pub fn new() -> Message {
        Message {
            buffer: Vec::new(),
            read_pos: 0,
        }
    }

    fn from_buffer(buffer: Vec<u8>) -> Message {
        Message {
            buffer,
            read_pos: 0,
        }
    }

    /// Appends 8-bit unsigned integer to the message
    pub fn write_u8(&mut self, n: u8) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 8-bit signed integer to the message
    pub fn write_i8(&mut self, n: i8) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 16-bit unsigned integer to the message
    pub fn write_u16(&mut self, n: u16) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 16-bit signed integer to the message
    pub fn write_i16(&mut self, n: i16) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 32-bit unsigned integer to the message
    pub fn write_u32(&mut self, n: u32) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 32-bit signed integer to the message
    pub fn write_i32(&mut self, n: i32) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 64-bit unsigned integer to the message
    pub fn write_u64(&mut self, n: u64) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 64-bit signed integer to the message
    pub fn write_i64(&mut self, n: i64) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 128-bit unsigned integer to the message
    pub fn write_u128(&mut self, n: u128) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 128-bit signed integer to the message
    pub fn write_i128(&mut self, n: i128) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 32-bit float to the message
    pub fn write_f32(&mut self, n: f32) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends 64-bit float to the message
    pub fn write_f64(&mut self, n: f64) {
        self.buffer.extend_from_slice(&n.to_le_bytes());
    }

    /// Appends buffer to the message
    pub fn write_buffer(&mut self, buf: &[u8]) {
        self.write_u32(buf.len() as u32);
        self.buffer.extend_from_slice(buf);
    }

    /// Reads 8-bit unsigned integer and moves read cursor
    /// # Returns
    /// `u8` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_u8(&mut self) -> Result<u8, MessageError> {
        if self.buffer.len() - self.read_pos < 1 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 1];
        self.read_pos += 1;
        Ok(u8::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 8-bit signed integer and moves read cursor
    /// # Returns
    /// `i8` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_i8(&mut self) -> Result<i8, MessageError> {
        if self.buffer.len() - self.read_pos < 1 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 1];
        self.read_pos += 1;
        Ok(i8::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 16-bit unsigned integer and moves read cursor
    /// # Returns
    /// `u16` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_u16(&mut self) -> Result<u16, MessageError> {
        if self.buffer.len() - self.read_pos < 2 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 2];
        self.read_pos += 2;
        Ok(u16::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 16-bit signed integer and moves read cursor
    /// # Returns
    /// `i16` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_i16(&mut self) -> Result<i16, MessageError> {
        if self.buffer.len() - self.read_pos < 2 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 2];
        self.read_pos += 2;
        Ok(i16::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 32-bit unsigned integer and moves read cursor
    /// # Returns
    /// `u32` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_u32(&mut self) -> Result<u32, MessageError> {
        if self.buffer.len() - self.read_pos < 4 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 4];
        self.read_pos += 4;
        Ok(u32::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 32-bit signed integer and moves read cursor
    /// # Returns
    /// `i32` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_i32(&mut self) -> Result<i32, MessageError> {
        if self.buffer.len() - self.read_pos < 4 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 4];
        self.read_pos += 4;
        Ok(i32::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 64-bit unsigned integer and moves read cursor
    /// # Returns
    /// `u64` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_u64(&mut self) -> Result<u64, MessageError> {
        if self.buffer.len() - self.read_pos < 8 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 8];
        self.read_pos += 8;
        Ok(u64::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 64-bit signed integer and moves read cursor
    /// # Returns
    /// `i64` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_i64(&mut self) -> Result<i64, MessageError> {
        if self.buffer.len() - self.read_pos < 8 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 8];
        self.read_pos += 8;
        Ok(i64::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 128-bit unsigned integer and moves read cursor
    /// # Returns
    /// `u128` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_u128(&mut self) -> Result<u128, MessageError> {
        if self.buffer.len() - self.read_pos < 16 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 16];
        self.read_pos += 16;
        Ok(u128::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 128-bit signed integer and moves read cursor
    /// # Returns
    /// `i128` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_i128(&mut self) -> Result<i128, MessageError> {
        if self.buffer.len() - self.read_pos < 16 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 16];
        self.read_pos += 16;
        Ok(i128::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 32-bit float and moves read cursor
    /// # Returns
    /// `f32` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_f32(&mut self) -> Result<f32, MessageError> {
        if self.buffer.len() - self.read_pos < 4 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 4];
        self.read_pos += 4;
        Ok(f32::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads 64-bit float and moves read cursor
    /// # Returns
    /// `f64` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_f64(&mut self) -> Result<f64, MessageError> {
        if self.buffer.len() - self.read_pos < 4 {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + 8];
        self.read_pos += 8;
        Ok(f64::from_le_bytes(slice.try_into().unwrap()))
    }

    /// Reads buffer and moves read cursor
    /// # Returns
    /// `Vec<u8>` or [MessageError](enum.MessageError.html) if reading failed
    pub fn read_buffer(&mut self) -> Result<Vec<u8>, MessageError> {
        let len = self.read_u32()? as usize;
        if self.buffer.len() - self.read_pos < len {
            return Err(UnexpectedEnd);
        }
        let slice = &self.buffer[self.read_pos..self.read_pos + len];
        self.read_pos += len;
        Ok(slice.to_vec())
    }
}
