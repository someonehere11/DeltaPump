extern crate rosc;

use std::net::{SocketAddrV4, UdpSocket};
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::f32;
use clap::Parser;

const BASE_PARAM: &str = "/avatar/parameters/";
const PUMP_ADDR: &str = "Pump/Pump_Stretch";
const DEFLATE_ADDR: &str = "Pooltoy/Deflate";
const INFLATE_ADDR: &str = "Pooltoy/Inflate";
const OVERINFLATE_ADDR: &str = "Pooltoy/Overinflate";

const SOUND_TRIGGER: &str = "Pump/Deltapump_Inflating";

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Address to listen to
    #[arg(short, long, default_value = "127.0.0.1:9009")]
    address: SocketAddrV4,

    /// Address to send to
    #[arg(short, long, default_value = "127.0.0.1:9000")]
    target_address: SocketAddrV4,

    /// Pump inflate modifier
    #[arg(short, long, default_value = "0.05")]
    pump_modifier: f32,
}


fn main() {
    let args: Args = Args::parse();

    let socket: UdpSocket = UdpSocket::bind(args.address).unwrap();
    
    println!("Listening to {}", args.address);
    println!("Sending to {}", args.target_address);

    let mut buf: [u8; 1536] = [0u8; rosc::decoder::MTU];

    let mut deflate_value: f32 = 0.0;
    let mut inflate_value: f32 = 0.0;
    let mut overinflate_value: f32 = 0.0;
    let mut last_pump_value: f32 = 0.0;

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                //println!("Received packet with size {} from: {}", size, addr);

                let (_, packet): (&[u8], OscPacket) = rosc::decoder::decode_udp(&buf[..size]).unwrap();
                
                handle_packet(packet, &socket, &mut deflate_value, &mut inflate_value, &mut overinflate_value, &mut last_pump_value, args.pump_modifier, args.target_address);
            }
            Err(e) => {
                println!("Error receiving from socket: {}", e);
                break;
            }
        }
    }
}

fn handle_packet(packet: OscPacket, sock: &UdpSocket, deflate_value: &mut f32, inflate_value: &mut f32, overinflate_value: &mut f32, last_pump_value: &mut f32, pump_modifier: f32, target_address: SocketAddrV4) {
    match packet {
        OscPacket::Message(msg) => {
            handle_message_received(msg, sock, deflate_value, inflate_value, overinflate_value, last_pump_value, pump_modifier, target_address);
        }
        OscPacket::Bundle(bundle) => {
            println!("OSC Bundle: {:?}", bundle);
        }
    }
}

fn handle_message_received(msg: OscMessage, sock: &UdpSocket, deflate_value: &mut f32, inflate_value: &mut f32, overinflate_value: &mut f32, last_pump_value: &mut f32, pump_modifier: f32, target_address: SocketAddrV4) {
    let value: OscType = msg.args[0].clone();
    
    if !matches!(value, OscType::Float(_)) {
        //println!("Unknown OSC message type: {:?}", msg);
        return;
    }

    let val: f32 = match value {
        OscType::Float(f) => f,
        _ => 0.0,
    };

    if msg.addr == format!("{BASE_PARAM}{DEFLATE_ADDR}") {
        println!("Deflate value: {}", val);
        *deflate_value = val;
    }

    else if msg.addr == format!("{BASE_PARAM}{INFLATE_ADDR}") {
        println!("Inflate value: {}", val);
        *inflate_value = val;
    }

    else if msg.addr == format!("{BASE_PARAM}{OVERINFLATE_ADDR}") {
        println!("Overinflate value: {}", val);
        *overinflate_value = val;
    }
    
    else if msg.addr == format!("{BASE_PARAM}{PUMP_ADDR}") {
        println!("Pump value: {}", val);
        pump_update(sock, val, *last_pump_value, deflate_value, inflate_value, overinflate_value, pump_modifier, target_address);
        *last_pump_value = val;
    }

    // else {
    //     println!("Unknown OSC message: {:?}", msg);
    // }
    
}

fn pump_update(sock: &UdpSocket, pump_position: f32, last_pump_position: f32, deflate_value: &mut f32, inflate_value: &mut f32, overinflate_value: &mut f32, pump_modifier: f32, target_address: SocketAddrV4) {

    let mut pump_delta: f32 = pump_position - last_pump_position;
    pump_delta = pump_delta.abs();

    if pump_delta < 0.01 {
        return;
    }

    let mut inflate_delta: f32 = pump_delta * pump_modifier;

    let mut new_deflate_value: f32 = *deflate_value;
    let mut new_overinflate_value: f32 = *overinflate_value;

    if *deflate_value > 0.0 {
        new_deflate_value = f32::max(0.0, new_deflate_value - inflate_delta);
        inflate_delta = f32::max(0.0, inflate_delta - *deflate_value);
    }

    let mut new_inflate_value: f32 = *inflate_value + inflate_delta;

    if new_inflate_value > 1.0 {
        new_inflate_value = 1.0;
        new_overinflate_value = f32::min(1.0, *overinflate_value + inflate_delta);
    }
       
    // Debug prints
    println!("\nPUMP UPDATE");
    println!("[Pump] Stretch delta: {pump_delta}");
    println!("[Pump] Inflate delta: {inflate_delta}");
    println!("[Pump] Deflate: {new_deflate_value}");
    println!("[Pump] Inflate: {new_inflate_value}");
    println!("[Pump] Overinflate: {new_overinflate_value}");

    // Update values on avi
    let send_addr: String = target_address.to_string();
    send_osc_value_f32(sock, format!("{BASE_PARAM}{DEFLATE_ADDR}"), send_addr.clone(), new_deflate_value);
    send_osc_value_f32(sock, format!("{BASE_PARAM}{INFLATE_ADDR}"), send_addr.clone(), new_inflate_value);
    send_osc_value_f32(sock, format!("{BASE_PARAM}{OVERINFLATE_ADDR}"), send_addr.clone(), new_overinflate_value);
    send_osc_value_bool(sock, format!("{BASE_PARAM}{SOUND_TRIGGER}"), send_addr.clone(), true);
}

fn send_osc_value_f32(sock: &UdpSocket, param: String, addr: String, value: f32) {
    let msg_buf: Vec<u8> = encoder::encode(&OscPacket::Message(OscMessage {
        addr: param,
        args: vec![OscType::Float(value)],
    }))
    .unwrap();

    sock.send_to(&msg_buf, addr).unwrap();
}

fn send_osc_value_bool(sock: &UdpSocket, param: String, addr: String, value: bool) {
    let msg_buf: Vec<u8> = encoder::encode(&OscPacket::Message(OscMessage {
        addr: param,
        args: vec![OscType::Bool(value)],
    }))
    .unwrap();

    sock.send_to(&msg_buf, addr).unwrap();
}