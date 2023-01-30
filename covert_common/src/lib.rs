#![warn(missing_docs)]
#![allow(incomplete_features)]
#![feature(slice_as_chunks)]
#![feature(generic_const_exprs)]
//! Helper utilities for creating [external c2][1] systems for [cobaltstrike][2].
//!
//! ![C2](https://i.ibb.co/Cszd81H/externalc2.png)
//!
//!
//!
//![1]: https://hstechdocs.helpsystems.com/manuals/cobaltstrike/current/userguide/content/topics/listener-infrastructue_external-c2.htm
//! [2]: https://www.cobaltstrike.com/

use aes::Aes256;
use bincode::{
    config::{RejectTrailing, VarintEncoding, WithOtherIntEncoding, WithOtherTrailing},
    DefaultOptions, Options,
};
use cipher::{
    block_padding::Pkcs7, generic_array::GenericArray, BlockDecrypt, BlockEncrypt,
    KeyInit,
};
use rand::random;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::error::Error;
use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
};

/// Error type for the things that can go wrong when you put packets in the channel
///

#[derive(Debug)]
pub enum CovertError {
    /// Theres an issue with decrypting the packet
    DecryptionError,
    /// There is an issue with the hash after decryption (Could be a tampered packet)
    InvalidHash,
    /// The contained message or packet could not be deserialized to a rust struct
    DeserializeError,
}

impl Display for CovertError {
    #[cfg(not(debug_assertions))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }

    #[cfg(debug_assertions)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CovertError::DecryptionError => write!(f, "Decryption error"),
            CovertError::InvalidHash => write!(f, "Invalid hash error"),
            CovertError::DeserializeError => write!(f, "Deserialization error"),
        }
    }
}

impl Error for CovertError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        ""
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct CovertPacket {
    hash: u8,
    stream: u16,
    syn: u32,
    want: u32,
    last: bool,
    payload: Vec<u8>,
    pad: Vec<u8>,
} //17 bytes max

impl CovertPacket {
    fn new(stream: u16, syn: u32, last: bool, payload: &[u8], blocks: usize) -> Self {
        let buf_size = (blocks * 16) - 18;
        let pad = vec![0u8; buf_size - payload.len()];
        let pad: Vec<u8> = pad.iter().map(|_| random()).collect();

        Self {
            hash: 0,
            stream,
            syn,
            want: 0,
            last,
            payload: payload.to_vec(),
            pad,
        }
    }
}

struct CovertStream<T> {
    out_bound_packet_cache: VecDeque<CovertPacket>,
    in_bound_packet_cache: VecDeque<CovertPacket>,
    message_cache: VecDeque<T>,
    out_count: u32,
    out_syn: u32,
    in_syn: u32,
}

/// Establishes a covert channel handler.  this struct enables sending series of messages
/// that are broken into small encrypted packets.  messages are put into the channel with
/// put_message and then sent with get_packet.  In the reverse direction packets are
/// read into the channel with put_packets and converted to messages with get_message.
/// packets need to be sent in both directions to "syn/ack" to confirm messages are
/// transmitted in full.

pub struct CovertChannel<T, const BLOCKS: usize>
where
    T: DeserializeOwned + Serialize,
{
    engine: Aes256,
    encoder: WithOtherTrailing<
        WithOtherIntEncoding<DefaultOptions, VarintEncoding>,
        RejectTrailing,
    >,
    streams: HashMap<u16, CovertStream<T>>,
}

impl<T, const BLOCKS: usize> CovertChannel<T, BLOCKS>
where
    T: DeserializeOwned + Serialize,
{
    /// Create a new covert channel with the provided 32 byte aes key.
    pub fn new(key: [u8; 32]) -> CovertChannel<T, BLOCKS> {
        let encoder = bincode::DefaultOptions::new()
            .with_varint_encoding()
            .reject_trailing_bytes();
        // let cryptor = with_key(create_key(key).expect("Bad key"));
        CovertChannel {
            encoder,
            engine: Aes256::new(&GenericArray::from(key)),
            streams: HashMap::new(),
        }
    }

    /// get a complete message T from this channel.  if there are no messages sent completely
    /// yet then the Option is None.
    pub fn get_message(&mut self, stream_id: u16) -> Option<T> {
        let stream = self.streams.get_mut(&stream_id)?;
        stream.message_cache.pop_front()
    }

    /// send a message through this channel.  
    pub fn put_message(&mut self, msg: T, stream_id: u16) -> ()
    where
        [(); (BLOCKS * 16) - 18]:,
    {
        let encode = self.encoder;
        let stream = self.get_stream_by_id(stream_id);
        let res = encode.serialize(&msg).unwrap();
        let (parts, end) = res.as_chunks::<{ (BLOCKS * 16) - 18 }>();
        for part in parts {
            let new_packet = CovertPacket::new(
                stream_id,
                stream.out_count,
                false,
                part.as_slice(),
                BLOCKS,
            );
            stream.out_count += 1;
            stream.out_bound_packet_cache.push_back(new_packet);
        }
        if end.len() != 0 {
            let last_packet =
                CovertPacket::new(stream_id, stream.out_count, true, end, BLOCKS);
            stream.out_bound_packet_cache.push_back(last_packet);
            stream.out_count += 1;
        } else {
            stream.out_bound_packet_cache.back_mut().unwrap().last = true;
        }
    }

    /// get the next packet that needs to be sent for this channel.  Even if there are
    /// no messages to be sent a call to get_packet with return successfully.  empty packets
    /// still contain synchronizing information and should be sent regularly.  additionally
    /// the receiving end of the channel accounts for packets that contain no message data.
    pub fn get_packet(&mut self, stream_id: u16) -> Vec<u8> {
        let encode = self.encoder;
        let stream = self.get_stream_by_id(stream_id);
        if stream.out_bound_packet_cache.len() == 0 {
            stream.out_bound_packet_cache.push_front(CovertPacket::new(
                stream_id,
                stream.out_count,
                false,
                &[],
                BLOCKS,
            ));
            stream.out_count += 1;
        }
        let out = stream.out_bound_packet_cache.front_mut().unwrap();
        out.want = stream.in_syn;
        let mut tmp = encode.serialize(out).unwrap();
        let hash = crc32fast::hash(&tmp).to_le_bytes()[0];
        tmp[0] = hash;
        let done = self.engine.encrypt_padded_vec::<Pkcs7>(&tmp);
        return done;
    }

    /// Put packets into this channel to be decoded.  When a complete packet is ready to
    /// be read from the channel put packet will contain true in the Ok result.
    pub fn put_packet(&mut self, pkt: &[u8]) -> Result<(u16, bool), CovertError> {
        let encode = self.encoder;
        let mut tmp = self
            .engine
            .decrypt_padded_vec::<Pkcs7>(pkt)
            .or(Err(CovertError::DecryptionError))?;
        let hash = tmp[0];
        tmp[0] = 0;
        let actual = crc32fast::hash(&tmp).to_le_bytes()[0];
        if hash != actual {
            return Err(CovertError::InvalidHash);
        };
        let in_packet = encode
            .deserialize::<CovertPacket>(&tmp)
            .or(Err(CovertError::DeserializeError))?;

        let stream_id = in_packet.stream;
        let mut stream = self.get_stream_by_id(stream_id);

        // Clear packets in the out_cache that have been confirmed
        stream.out_syn = Ord::max(stream.out_syn, in_packet.want);
        stream
            .out_bound_packet_cache
            .retain(|item| item.syn >= stream.out_syn);
        // Append to the in_cache if this is the next packet needed else drop it
        if in_packet.syn == stream.in_syn {
            stream.in_syn += 1;
            let is_last = in_packet.last;
            stream.in_bound_packet_cache.push_back(in_packet);
            if is_last {
                let mut payload: Vec<u8> = stream
                    .in_bound_packet_cache
                    .drain(..)
                    .map(|i| i.payload)
                    .flatten()
                    .collect();
                let in_message = encode
                    .deserialize::<T>(&mut payload)
                    .or(Err(CovertError::DeserializeError))?;
                stream.message_cache.push_back(in_message);
                return Ok((stream_id, true));
            }
        };
        return Ok((stream_id, false));
    }

    /// Get the number of packets in the inbound and outbound queue (in,out)
    pub fn packets_in_queue(&self, stream_id: u16) -> (usize, usize) {
        if let Some(stream) = self.streams.get(&stream_id) {
            return (
                stream.in_bound_packet_cache.len(),
                stream.out_bound_packet_cache.len(),
            );
        }
        return (0, 0);
    }

    fn get_stream_by_id(&mut self, stream_id: u16) -> &mut CovertStream<T> {
        if !self.streams.contains_key(&stream_id) {
            let new_stream = CovertStream {
                out_bound_packet_cache: VecDeque::new(),
                in_bound_packet_cache: VecDeque::new(),
                message_cache: VecDeque::new(),
                out_count: 0,
                out_syn: 0,
                in_syn: 0,
            };
            self.streams.insert(stream_id, new_stream);
        }
        self.streams.get_mut(&stream_id).unwrap()
    }
}

#[cfg(test)]
mod test;
